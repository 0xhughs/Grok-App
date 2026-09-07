"""Offline tests for scripts/check_workflow_pins.py (unittest, stdlib only).

`subprocess` is stubbed for every test, so any code path that would shell
out to `git ls-remote` fails the test instead of touching the network.

Run: python3 -m unittest scripts/check_workflow_pins_test.py
"""

from __future__ import annotations

import io
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPTS_DIR = Path(__file__).resolve().parent
ROOT = SCRIPTS_DIR.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import check_workflow_pins as cwp  # noqa: E402

CI_YML = ROOT / ".github/workflows/ci.yml"
RELEASE_YML = ROOT / ".github/workflows/release.yml"

# Fixture listing: verbatim `git ls-remote --tags --heads` pairs captured at
# build time (slice 01 Proof). Annotated tags carry both the tag-object line
# and the peeled `^{}` line, exactly as git prints them.
LIVE_LISTING = {
    "actions/checkout": [
        ("f548e57e544e1ff5a4c46bf1e1b8685f8e4a348a", "refs/heads/main"),
        ("eef61447b9ff4aafe5dcd4e0bbf5d482be7e7871", "refs/tags/v4.2.1"),
        ("11bd71901bbe5b1630ceea73d27597364c9af683", "refs/tags/v4.2.2"),
    ],
    "pnpm/action-setup": [
        ("ea17c68df8912ef543352723c149a84f56e3d413", "refs/heads/master"),
        ("0c17529a66aca453f9227af23103ed11469b1e47", "refs/tags/v4.0.0"),
        ("fe02b34f77f8bc703788d5817da081398fad5dd2", "refs/tags/v4.0.0^{}"),
    ],
    "actions/setup-node": [
        ("94196ee1d15439c1b6651cd87ef14e88ec435966", "refs/heads/main"),
        ("1e60f620b9541d16bece96c5465dc8ee9832be0b", "refs/tags/v4.0.3"),
    ],
    "swatinem/rust-cache": [
        ("f0d9c3887740aee45f6153b24b3a6b815192ec16", "refs/heads/master"),
        ("400e7407cfd7a091e5fbb6afec01ec146c432b7c", "refs/tags/v2.7.7"),
        ("f0deed1e0edfc6a9be95417288c0e1099b1eeec3", "refs/tags/v2.7.7^{}"),
    ],
    "tauri-apps/tauri-action": [
        ("a6e90ddc4ba4721f294e52b856d3d50e645edc07", "refs/heads/dev"),
        ("ea1f12403fd95c98ee2fff70a34d5a818ceeb112", "refs/tags/v0.5.17"),
        ("2a8db2c169af2fdc695133781e27ecba52daea75", "refs/tags/v0.5.17^{}"),
    ],
    "dtolnay/rust-toolchain": [
        ("d1031067263f94b142dd6c0ce24c5eb9d02d52a0", "refs/heads/master"),
        ("6bed0761d98439e5a578e2877258200ad565ba87", "refs/heads/stable"),
    ],
}

# Convenience lookups derived from the fixture (never typed separately).
_FIX = {repo: {ref: sha for sha, ref in pairs} for repo, pairs in LIVE_LISTING.items()}
CHECKOUT_V422 = _FIX["actions/checkout"]["refs/tags/v4.2.2"]
CHECKOUT_V421 = _FIX["actions/checkout"]["refs/tags/v4.2.1"]
PNPM_V400_PEELED = _FIX["pnpm/action-setup"]["refs/tags/v4.0.0^{}"]
PNPM_V400_TAG_OBJECT = _FIX["pnpm/action-setup"]["refs/tags/v4.0.0"]
SETUP_NODE_V403 = _FIX["actions/setup-node"]["refs/tags/v4.0.3"]
RUST_CACHE_V277_PEELED = _FIX["swatinem/rust-cache"]["refs/tags/v2.7.7^{}"]
RUST_CACHE_V277_TAG_OBJECT = _FIX["swatinem/rust-cache"]["refs/tags/v2.7.7"]
TAURI_V0517_PEELED = _FIX["tauri-apps/tauri-action"]["refs/tags/v0.5.17^{}"]
DTOLNAY_STABLE = _FIX["dtolnay/rust-toolchain"]["refs/heads/stable"]


def fixture_list_refs(owner_repo: str) -> list[tuple[str, str]]:
    try:
        return list(LIVE_LISTING[owner_repo.lower()])
    except KeyError as exc:
        raise cwp.ListingError(f"{owner_repo} not in fixture") from exc


def flip_hex(sha: str, index: int = 0) -> str:
    ch = sha[index]
    new = "1" if ch == "0" else "0"
    return sha[:index] + new + sha[index + 1 :]


def _forbidden(*args, **kwargs):  # pragma: no cover — should never be reached
    raise AssertionError(f"subprocess must not be used in tests: {args} {kwargs}")


class _NoNetworkCase(unittest.TestCase):
    """Base class that stubs subprocess so no test can reach `git`."""

    def setUp(self) -> None:
        for name in ("run", "Popen", "check_output", "check_call", "call"):
            patcher = mock.patch.object(subprocess, name, side_effect=_forbidden)
            patcher.start()
            self.addCleanup(patcher.stop)
        self.tmp = Path(tempfile.mkdtemp(prefix="pins-test-"))
        self.addCleanup(shutil.rmtree, self.tmp, True)

    def copy_workflow(self, src: Path, replace: list[tuple[str, str]] | None = None) -> Path:
        text = src.read_text(encoding="utf-8")
        for old, new in replace or []:
            self.assertIn(old, text, f"fixture edit target missing: {old!r}")
            text = text.replace(old, new, 1)
        dst = self.tmp / src.name
        dst.write_text(text, encoding="utf-8")
        return dst

    def run_cli(self, argv: list[str]) -> tuple[int, str]:
        out = io.StringIO()
        with mock.patch.object(sys, "stdout", out):
            code = cwp.main(argv)
        return code, out.getvalue()

    def write_refs_json(self, data: dict, name: str = "refs.json") -> Path:
        path = self.tmp / name
        path.write_text(json.dumps(data), encoding="utf-8")
        return path

    @staticmethod
    def verdicts(results: list[cwp.Result]) -> list[str]:
        return [r.verdict for r in results]


class SubprocessStubTest(_NoNetworkCase):
    def test_default_lister_is_blocked_and_reports_network(self) -> None:
        # Sanity check of the stub itself: reaching git is a NETWORK failure,
        # never UNKNOWN_SHA, and never a real network call.
        pins = cwp.parse_workflow_pins(CI_YML)
        results = cwp.check_pins(pins, cwp.git_list_refs)
        self.assertTrue(results)
        self.assertEqual(set(self.verdicts(results)), {"NETWORK"})
        self.assertEqual(cwp.exit_code_for(results), 2)


class ParseTest(_NoNetworkCase):
    def test_parses_both_shapes_and_all_fourteen_pins(self) -> None:
        ci = cwp.parse_workflow_pins(CI_YML)
        rel = cwp.parse_workflow_pins(RELEASE_YML)
        self.assertEqual(len(ci), 6)
        self.assertEqual(len(rel), 8)
        # release.yml uses `- name:` then `uses:` on the next line.
        self.assertTrue(all(p.repo is not None for p in ci + rel), [p.raw for p in ci + rel if p.repo is None])
        repos = {cwp.owner_repo_of(p) for p in ci + rel}
        self.assertEqual(set(LIVE_LISTING), repos)

    def test_label_grammar(self) -> None:
        self.assertEqual(cwp.parse_label("v4.2.2"), "refs/tags/v4.2.2")
        self.assertEqual(cwp.parse_label("stable (refs/heads/stable)"), "refs/heads/stable")
        self.assertIsNone(cwp.parse_label("stable (refs/heads/main)"))
        self.assertIsNone(cwp.parse_label("v4.2.2 pinned"))
        self.assertIsNone(cwp.parse_label(""))

    def test_subpath_action_is_well_formed(self) -> None:
        pin = cwp.parse_uses_value("f", 1, f"github/codeql-action/init@{CHECKOUT_V422} # v3.1.0")
        self.assertEqual(pin.repo, "github/codeql-action/init")
        self.assertEqual(cwp.owner_repo_of(pin), "github/codeql-action")
        self.assertEqual(pin.ref, "refs/tags/v3.1.0")

    def test_commented_out_uses_lines_are_ignored(self) -> None:
        wf = self.tmp / "c.yml"
        wf.write_text(f"steps:\n  # - uses: actions/checkout@v4\n  - uses: actions/checkout@{CHECKOUT_V422} # v4.2.2\n")
        pins = cwp.parse_workflow_pins(wf)
        self.assertEqual(len(pins), 1)
        self.assertEqual(pins[0].line, 3)


class CaseARepairedWorkflowsTest(_NoNetworkCase):
    def test_all_pins_ok_with_fixture_listing(self) -> None:
        calls: list[str] = []

        def counting(owner_repo: str):
            calls.append(owner_repo)
            return fixture_list_refs(owner_repo)

        pins = cwp.parse_workflow_pins(CI_YML) + cwp.parse_workflow_pins(RELEASE_YML)
        results = cwp.check_pins(pins, counting)
        self.assertEqual(len(results), 14)
        self.assertEqual(set(self.verdicts(results)), {"OK"}, [r.format() for r in results if r.verdict != "OK"])
        self.assertEqual(cwp.exit_code_for(results), 0)
        # One listing per distinct repo.
        self.assertEqual(sorted(calls), sorted(LIVE_LISTING))
        for r in results:
            self.assertIn(f"[{r.pin.ref} = {r.pin.sha}]", r.format())

    def test_cli_refs_json_on_real_files_exits_zero(self) -> None:
        refs = self.write_refs_json(LIVE_LISTING)
        code, out = self.run_cli(["--refs-json", str(refs), str(CI_YML), str(RELEASE_YML)])
        self.assertEqual(code, 0, out)
        lines = [l for l in out.splitlines() if " -> " in l]
        self.assertEqual(len(lines), 14)
        self.assertTrue(all(" -> OK [" in l for l in lines), out)
        self.assertIn(".github/workflows/ci.yml:17 actions/checkout@", out)
        self.assertIn(" # stable (refs/heads/stable) -> OK [refs/heads/stable = " + DTOLNAY_STABLE + "]", out)

    def test_cli_default_files_are_the_two_workflows(self) -> None:
        refs = self.write_refs_json(LIVE_LISTING)
        code, out = self.run_cli(["--refs-json", str(refs)])
        self.assertEqual(code, 0, out)
        self.assertIn("pins=14 OK=14 exit=0", out)


class CaseBUnknownShaTest(_NoNetworkCase):
    def test_flipped_hex_digit_is_unknown_sha(self) -> None:
        bad = flip_hex(SETUP_NODE_V403)
        wf = self.copy_workflow(CI_YML, [(SETUP_NODE_V403, bad)])
        results = cwp.check_pins(cwp.parse_workflow_pins(wf), fixture_list_refs)
        by_verdict = {r.verdict for r in results}
        self.assertEqual(by_verdict, {"OK", "UNKNOWN_SHA"})
        hit = [r for r in results if r.verdict == "UNKNOWN_SHA"]
        self.assertEqual(len(hit), 1)
        self.assertEqual(hit[0].pin.sha, bad)
        self.assertIn(f"refs/tags/v4.0.3 = {SETUP_NODE_V403}", hit[0].detail)
        self.assertEqual(cwp.exit_code_for(results), 1)

    def test_cli_exit_one(self) -> None:
        bad = flip_hex(CHECKOUT_V422, 39)
        wf = self.copy_workflow(RELEASE_YML, [(CHECKOUT_V422, bad)])
        refs = self.write_refs_json(LIVE_LISTING)
        code, out = self.run_cli(["--refs-json", str(refs), str(wf)])
        self.assertEqual(code, 1, out)
        self.assertIn(f"actions/checkout@{bad} # v4.2.2 -> UNKNOWN_SHA [", out)


class CaseCTagObjectTest(_NoNetworkCase):
    def test_tag_object_id_is_rejected(self) -> None:
        wf = self.copy_workflow(CI_YML, [(PNPM_V400_PEELED, PNPM_V400_TAG_OBJECT)])
        results = cwp.check_pins(cwp.parse_workflow_pins(wf), fixture_list_refs)
        hit = [r for r in results if r.pin.sha == PNPM_V400_TAG_OBJECT]
        self.assertEqual(len(hit), 1)
        self.assertEqual(hit[0].verdict, "TAG_OBJECT")
        self.assertIn(f"refs/tags/v4.0.0 = {PNPM_V400_PEELED}", hit[0].detail)
        self.assertEqual(cwp.exit_code_for(results), 1)

    def test_tag_object_wins_over_ref_mismatch(self) -> None:
        # Tag object of v2.7.7 with a comment naming a different ref.
        wf = self.tmp / "w.yml"
        wf.write_text(f"steps:\n  - uses: swatinem/rust-cache@{RUST_CACHE_V277_TAG_OBJECT} # v2.7.6\n")
        results = cwp.check_pins(cwp.parse_workflow_pins(wf), fixture_list_refs)
        self.assertEqual(self.verdicts(results), ["TAG_OBJECT"])

    def test_cli_exit_one(self) -> None:
        wf = self.copy_workflow(RELEASE_YML, [(TAURI_V0517_PEELED, _FIX["tauri-apps/tauri-action"]["refs/tags/v0.5.17"])])
        refs = self.write_refs_json(LIVE_LISTING)
        code, out = self.run_cli(["--refs-json", str(refs), str(wf)])
        self.assertEqual(code, 1, out)
        self.assertIn("-> TAG_OBJECT [", out)


class CaseDMalformedTest(_NoNetworkCase):
    def _check(self, value: str) -> cwp.Result:
        wf = self.tmp / "m.yml"
        wf.write_text(f"steps:\n  - uses: {value}\n")
        results = cwp.check_pins(cwp.parse_workflow_pins(wf), fixture_list_refs)
        self.assertEqual(len(results), 1)
        return results[0]

    def test_floating_tag(self) -> None:
        r = self._check("actions/checkout@v4")
        self.assertEqual(r.verdict, "MALFORMED")
        self.assertIn("actions/checkout@v4 -> MALFORMED [", r.format())

    def test_missing_comment(self) -> None:
        self.assertEqual(self._check(f"actions/checkout@{CHECKOUT_V422}").verdict, "MALFORMED")

    def test_other_malformed_shapes(self) -> None:
        cases = [
            f"actions/checkout@{CHECKOUT_V422[:12]} # v4.2.2",  # short sha
            f"actions/checkout@{CHECKOUT_V422.upper()} # v4.2.2",  # uppercase hex
            "docker://alpine:3.19 # 3.19",
            "./.github/actions/local # local",
            "dtolnay/rust-toolchain@stable # stable",
            f"actions/checkout@{CHECKOUT_V422} # v4.2.2 extra words",  # unknown comment shape
            f"dtolnay/rust-toolchain@{DTOLNAY_STABLE} # stable (refs/heads/master)",  # name mismatch inside label
            f"actions/checkout@{CHECKOUT_V422} #",  # empty comment
        ]
        for value in cases:
            with self.subTest(value=value):
                self.assertEqual(self._check(value).verdict, "MALFORMED")

    def test_malformed_pins_do_not_call_list_refs_and_exit_one(self) -> None:
        wf = self.tmp / "m.yml"
        wf.write_text(f"steps:\n  - uses: actions/checkout@v4\n  - uses: actions/checkout@{CHECKOUT_V422}\n")
        called: list[str] = []

        def lister(owner_repo: str):
            called.append(owner_repo)
            return fixture_list_refs(owner_repo)

        results = cwp.check_pins(cwp.parse_workflow_pins(wf), lister)
        self.assertEqual(self.verdicts(results), ["MALFORMED", "MALFORMED"])
        self.assertEqual(called, [])
        self.assertEqual(cwp.exit_code_for(results), 1)


class CaseERefMismatchTest(_NoNetworkCase):
    def test_real_sha_with_wrong_ref_comment(self) -> None:
        wf = self.tmp / "r.yml"
        wf.write_text(f"steps:\n  - uses: actions/checkout@{CHECKOUT_V421} # v4.2.2\n")
        results = cwp.check_pins(cwp.parse_workflow_pins(wf), fixture_list_refs)
        self.assertEqual(self.verdicts(results), ["REF_MISMATCH"])
        self.assertIn(f"[refs/tags/v4.2.2 = {CHECKOUT_V422}]", results[0].format())
        self.assertEqual(cwp.exit_code_for(results), 1)

    def test_real_sha_with_absent_ref_comment(self) -> None:
        wf = self.tmp / "r.yml"
        wf.write_text(f"steps:\n  - uses: actions/checkout@{CHECKOUT_V422} # v9.9.9\n")
        results = cwp.check_pins(cwp.parse_workflow_pins(wf), fixture_list_refs)
        self.assertEqual(self.verdicts(results), ["REF_MISMATCH"])
        self.assertIn("refs/tags/v9.9.9 not in listing", results[0].detail)

    def test_branch_tip_with_tag_comment_is_mismatch(self) -> None:
        wf = self.tmp / "r.yml"
        wf.write_text(f"steps:\n  - uses: dtolnay/rust-toolchain@{DTOLNAY_STABLE} # stable\n")
        results = cwp.check_pins(cwp.parse_workflow_pins(wf), fixture_list_refs)
        # `# stable` means refs/tags/stable, which does not exist upstream.
        self.assertEqual(self.verdicts(results), ["REF_MISMATCH"])


class CaseFNetworkTest(_NoNetworkCase):
    def test_list_refs_raising_is_network_exit_two(self) -> None:
        def flaky(owner_repo: str):
            if owner_repo == "actions/setup-node":
                raise cwp.ListingError("git ls-remote: exit 128 could not resolve host")
            return fixture_list_refs(owner_repo)

        results = cwp.check_pins(cwp.parse_workflow_pins(CI_YML), flaky)
        affected = [r for r in results if cwp.owner_repo_of(r.pin) == "actions/setup-node"]
        self.assertEqual(len(affected), 1)
        self.assertEqual(affected[0].verdict, "NETWORK")
        self.assertNotEqual(affected[0].verdict, "UNKNOWN_SHA")
        self.assertIn("could not resolve host", affected[0].detail)
        others = {r.verdict for r in results if r not in affected}
        self.assertEqual(others, {"OK"})
        self.assertEqual(cwp.exit_code_for(results), 2)

    def test_generic_exception_from_lister_is_network(self) -> None:
        def broken(owner_repo: str):
            raise RuntimeError("boom")

        results = cwp.check_pins(cwp.parse_workflow_pins(CI_YML), broken)
        self.assertEqual(set(self.verdicts(results)), {"NETWORK"})
        self.assertEqual(cwp.exit_code_for(results), 2)

    def test_unparsable_listing_is_network(self) -> None:
        with self.assertRaises(cwp.ListingError):
            cwp.parse_ls_remote_output("fatal: not a git repository\n")
        with self.assertRaises(cwp.ListingError):
            cwp.parse_ls_remote_output("")

    def test_refs_json_error_object_exits_two(self) -> None:
        data = dict(LIVE_LISTING)
        data["swatinem/rust-cache"] = {"error": "timeout after 60s"}
        refs = self.write_refs_json(data)
        code, out = self.run_cli(["--refs-json", str(refs), str(CI_YML)])
        self.assertEqual(code, 2, out)
        self.assertIn("swatinem/rust-cache@", out)
        self.assertIn("-> NETWORK [listing failed: timeout after 60s]", out)
        self.assertNotIn("UNKNOWN_SHA", out)

    def test_refs_json_missing_repo_exits_two(self) -> None:
        data = {k: v for k, v in LIVE_LISTING.items() if k != "dtolnay/rust-toolchain"}
        refs = self.write_refs_json(data)
        code, out = self.run_cli(["--refs-json", str(refs), str(CI_YML)])
        self.assertEqual(code, 2, out)
        self.assertIn("dtolnay/rust-toolchain@", out)
        self.assertIn("-> NETWORK [", out)
        self.assertNotIn("UNKNOWN_SHA", out)

    def test_network_takes_precedence_over_exit_one(self) -> None:
        bad = flip_hex(CHECKOUT_V422)
        wf = self.copy_workflow(CI_YML, [(CHECKOUT_V422, bad)])
        data = dict(LIVE_LISTING)
        data["pnpm/action-setup"] = {"error": "connection reset"}
        refs = self.write_refs_json(data)
        code, out = self.run_cli(["--refs-json", str(refs), str(wf)])
        self.assertEqual(code, 2, out)
        self.assertIn("-> UNKNOWN_SHA [", out)
        self.assertIn("-> NETWORK [", out)


class ExitCodeTest(_NoNetworkCase):
    def test_precedence_table(self) -> None:
        def res(verdict: str) -> cwp.Result:
            return cwp.Result(cwp.Pin("f", 1, "x"), verdict, "")

        self.assertEqual(cwp.exit_code_for([res("OK")]), 0)
        for v in ("MALFORMED", "UNKNOWN_SHA", "TAG_OBJECT", "REF_MISMATCH"):
            self.assertEqual(cwp.exit_code_for([res("OK"), res(v)]), 1, v)
        self.assertEqual(cwp.exit_code_for([res("MALFORMED"), res("NETWORK"), res("OK")]), 2)
        self.assertEqual(cwp.exit_code_for([]), 0)

    def test_no_uses_lines_fails_closed(self) -> None:
        wf = self.tmp / "empty.yml"
        wf.write_text("name: nothing\n")
        refs = self.write_refs_json(LIVE_LISTING)
        code, out = self.run_cli(["--refs-json", str(refs), str(wf)])
        self.assertEqual(code, 1, out)


if __name__ == "__main__":
    unittest.main()
