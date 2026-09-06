#!/usr/bin/env python3
"""Verify that every GitHub Actions `uses:` pin is the real commit of a named ref.

Usage:
  python3 scripts/check_workflow_pins.py                       # both workflows, live
  python3 scripts/check_workflow_pins.py path/to/a.yml [...]   # explicit files
  python3 scripts/check_workflow_pins.py --refs-json refs.json # offline listing

Pin format (normative):
  uses: <owner>/<repo>[/<subpath>]@<40 lowercase hex> # <ref-label>
  <ref-label> is `<tag>`                       -> refs/tags/<tag>
           or `<name> (refs/heads/<name>)`    -> refs/heads/<name>

The pinned SHA must be the *peeled commit id* of that ref as listed by
`git ls-remote --tags --heads https://github.com/<owner>/<repo>`: the
`refs/tags/<tag>^{}` line for an annotated tag, the ref's own line for a
lightweight tag or a branch. A tag-object id is never a valid pin.

Verdicts (classified in this order, exactly one per pin):
  MALFORMED     line does not match the pin format / comment grammar
  NETWORK       the repo's ref listing failed (fail closed)
  TAG_OBJECT    SHA is an annotated tag object, not its peeled commit
  UNKNOWN_SHA   SHA is neither a ref tip nor a peeled id in the listing
  REF_MISMATCH  SHA is real but is not the id of the ref named in the comment
  OK

Exit codes: 0 all OK; 1 any MALFORMED/UNKNOWN_SHA/TAG_OBJECT/REF_MISMATCH;
2 any NETWORK (takes precedence over 1). Stdlib only; one `git ls-remote`
per distinct repo.

`--refs-json` shape: {"owner/repo": [[sha, refname], ...] | {"error": "msg"}}.
A repo missing from the JSON counts as a failed listing (NETWORK).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable, Sequence

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_FILES = (
    ROOT / ".github/workflows/ci.yml",
    ROOT / ".github/workflows/release.yml",
)
LS_REMOTE_TIMEOUT_S = 60

VERDICT_ORDER = ("MALFORMED", "NETWORK", "TAG_OBJECT", "UNKNOWN_SHA", "REF_MISMATCH", "OK")
EXIT1_VERDICTS = frozenset({"MALFORMED", "TAG_OBJECT", "UNKNOWN_SHA", "REF_MISMATCH"})

# `uses:` may be a list item (`- uses:`) or a mapping key after `- name:`.
_USES_LINE_RE = re.compile(r"^\s*(?:-\s+)?uses:\s*(?P<value>.*?)\s*$")
_NAME_PART = r"[A-Za-z0-9_.-]+"
_PIN_RE = re.compile(
    r"^(?P<q>[\"']?)"
    rf"(?P<repo>{_NAME_PART}/{_NAME_PART})"
    r"(?P<subpath>(?:/[^@\s#\"']+)*)"
    r"@(?P<sha>[0-9a-f]{40})"
    r"(?P=q)"
    r"[ \t]+#[ \t]*(?P<label>\S.*?)$"
)
_REF_NAME = r"[^\s()#]+"
_TAG_LABEL_RE = re.compile(rf"^(?P<tag>{_REF_NAME})$")
_BRANCH_LABEL_RE = re.compile(rf"^(?P<name>{_REF_NAME}) \(refs/heads/(?P=name)\)$")
_LS_REMOTE_LINE_RE = re.compile(r"^(?P<sha>[0-9a-f]{40})\t(?P<ref>refs/\S+)$")

RefPair = tuple[str, str]
ListRefs = Callable[[str], Sequence[RefPair]]


class ListingError(Exception):
    """A ref listing could not be obtained or parsed (NETWORK verdict)."""


@dataclass(frozen=True)
class Pin:
    file: str
    line: int
    raw: str
    repo: str | None = None  # owner/repo[/subpath] as written
    sha: str | None = None
    label: str | None = None
    ref: str | None = None  # refs/tags/<tag> or refs/heads/<name>


@dataclass(frozen=True)
class Result:
    pin: Pin
    verdict: str
    detail: str

    def format(self) -> str:
        p = self.pin
        if p.repo is None:
            head = p.raw
        else:
            head = f"{p.repo}@{p.sha} # {p.label}"
        return f"{p.file}:{p.line} {head} -> {self.verdict} [{self.detail}]"


def parse_label(label: str) -> str | None:
    """Map a pin comment to the full ref name, or None if the shape is unknown."""
    m = _BRANCH_LABEL_RE.match(label)
    if m:
        return f"refs/heads/{m.group('name')}"
    m = _TAG_LABEL_RE.match(label)
    if m:
        return f"refs/tags/{m.group('tag')}"
    return None


def parse_uses_value(file: str, line_no: int, value: str) -> Pin:
    m = _PIN_RE.match(value)
    if not m:
        return Pin(file=file, line=line_no, raw=value)
    ref = parse_label(m.group("label"))
    if ref is None:
        return Pin(file=file, line=line_no, raw=value)
    return Pin(
        file=file,
        line=line_no,
        raw=value,
        repo=m.group("repo") + m.group("subpath"),
        sha=m.group("sha"),
        label=m.group("label"),
        ref=ref,
    )


def parse_workflow_pins(path: Path, display: str | None = None) -> list[Pin]:
    """Line-based scan for `uses:` entries (YAML comments skipped)."""
    display = display or str(path)
    pins: list[Pin] = []
    text = path.read_text(encoding="utf-8", errors="replace")
    for i, line in enumerate(text.splitlines(), 1):
        if line.lstrip().startswith("#"):
            continue
        m = _USES_LINE_RE.match(line)
        if not m:
            continue
        pins.append(parse_uses_value(display, i, m.group("value")))
    return pins


def owner_repo_of(pin: Pin) -> str:
    """`owner/repo` (lowercase) of a well-formed pin; strips any subpath."""
    assert pin.repo is not None
    owner, repo = pin.repo.split("/")[:2]
    return f"{owner}/{repo}".lower()


def parse_ls_remote_output(text: str) -> list[RefPair]:
    pairs: list[RefPair] = []
    for raw in text.splitlines():
        line = raw.rstrip("\r")
        if not line:
            continue
        m = _LS_REMOTE_LINE_RE.match(line)
        if not m:
            raise ListingError(f"unparsable ls-remote line: {line[:120]!r}")
        pairs.append((m.group("sha"), m.group("ref")))
    if not pairs:
        raise ListingError("ls-remote returned no refs")
    return pairs


def git_list_refs(owner_repo: str, timeout: float = LS_REMOTE_TIMEOUT_S) -> list[RefPair]:
    """Default lister: `git ls-remote --tags --heads https://github.com/<owner>/<repo>`."""
    url = f"https://github.com/{owner_repo}"
    cmd = ["git", "ls-remote", "--tags", "--heads", url]
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise ListingError(f"{' '.join(cmd)}: {exc.__class__.__name__}: {exc}") from exc
    if proc.returncode != 0:
        err = (proc.stderr or "").strip().splitlines()
        tail = err[-1] if err else ""
        raise ListingError(f"{' '.join(cmd)}: exit {proc.returncode} {tail}".rstrip())
    return parse_ls_remote_output(proc.stdout)


@dataclass
class Listing:
    """Indexed view of one repo's `git ls-remote --tags --heads` output."""

    by_ref: dict[str, str]
    tag_object_ids: dict[str, str]  # tag-object sha -> refs/tags/<tag>
    valid_ids: frozenset[str]  # branch tips, lightweight tags, peeled ids

    @classmethod
    def from_pairs(cls, pairs: Iterable[RefPair]) -> "Listing":
        by_ref: dict[str, str] = {}
        for sha, ref in pairs:
            if not re.fullmatch(r"[0-9a-f]{40}", sha) or not ref.startswith("refs/"):
                raise ListingError(f"bad ref pair: {(sha, ref)!r}")
            by_ref[ref] = sha
        tag_objects: dict[str, str] = {}
        valid: set[str] = set()
        for ref, sha in by_ref.items():
            if ref.endswith("^{}"):
                valid.add(sha)
                continue
            if f"{ref}^{{}}" in by_ref:
                tag_objects[sha] = ref
            else:
                valid.add(sha)
        return cls(by_ref=by_ref, tag_object_ids=tag_objects, valid_ids=frozenset(valid))

    def resolve(self, ref: str) -> str | None:
        """Peeled commit id for `ref`, or None if the ref is not listed."""
        peeled = self.by_ref.get(f"{ref}^{{}}")
        if peeled is not None:
            return peeled
        return self.by_ref.get(ref)


def classify(pin: Pin, listing: Listing | ListingError | None) -> Result:
    if pin.repo is None:
        return Result(pin, "MALFORMED", "expected owner/repo[/subpath]@<40 lowercase hex> # <ref-label>")
    if listing is None or isinstance(listing, ListingError):
        return Result(pin, "NETWORK", f"listing failed: {listing}")
    assert pin.sha is not None and pin.ref is not None
    resolved = listing.resolve(pin.ref)
    resolved_txt = f"{pin.ref} = {resolved}" if resolved else f"{pin.ref} not in listing"
    if pin.sha in listing.tag_object_ids:
        tag_ref = listing.tag_object_ids[pin.sha]
        return Result(
            pin,
            "TAG_OBJECT",
            f"{pin.sha} is the tag object of {tag_ref}; {tag_ref} = {listing.resolve(tag_ref)}",
        )
    if pin.sha not in listing.valid_ids:
        return Result(pin, "UNKNOWN_SHA", f"sha matches no ref tip or peeled id; {resolved_txt}")
    if resolved != pin.sha:
        return Result(pin, "REF_MISMATCH", resolved_txt)
    return Result(pin, "OK", resolved_txt)


def check_pins(pins: Sequence[Pin], list_refs: ListRefs) -> list[Result]:
    """Classify every pin; `list_refs` is called once per distinct owner/repo."""
    cache: dict[str, Listing | ListingError] = {}
    results: list[Result] = []
    for pin in pins:
        if pin.repo is None:
            results.append(classify(pin, None))
            continue
        key = owner_repo_of(pin)
        if key not in cache:
            try:
                cache[key] = Listing.from_pairs(list_refs(key))
            except Exception as exc:  # noqa: BLE001 — any lister failure is NETWORK
                cache[key] = exc if isinstance(exc, ListingError) else ListingError(
                    f"{exc.__class__.__name__}: {exc}"
                )
        results.append(classify(pin, cache[key]))
    return results


def exit_code_for(results: Sequence[Result]) -> int:
    verdicts = {r.verdict for r in results}
    if "NETWORK" in verdicts:
        return 2
    if verdicts & EXIT1_VERDICTS:
        return 1
    return 0


def load_refs_json(path: Path) -> ListRefs:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("--refs-json must be an object keyed by owner/repo")
    table = {str(k).lower(): v for k, v in data.items()}

    def list_refs(owner_repo: str) -> list[RefPair]:
        entry = table.get(owner_repo.lower())
        if entry is None:
            raise ListingError(f"{owner_repo} absent from refs JSON")
        if isinstance(entry, dict):
            raise ListingError(str(entry.get("error", "listing marked failed")))
        return [(str(sha), str(ref)) for sha, ref in entry]

    return list_refs


def run(
    files: Sequence[Path],
    list_refs: ListRefs,
    out=None,
    display_names: Sequence[str] | None = None,
) -> int:
    out = out if out is not None else sys.stdout
    pins: list[Pin] = []
    for idx, path in enumerate(files):
        display = display_names[idx] if display_names else _display(path)
        pins.extend(parse_workflow_pins(path, display))
    if not pins:
        print("no `uses:` lines found — nothing verified", file=out)
        return 1
    results = check_pins(pins, list_refs)
    for r in results:
        print(r.format(), file=out)
    code = exit_code_for(results)
    counts = {v: sum(1 for r in results if r.verdict == v) for v in VERDICT_ORDER}
    summary = ", ".join(f"{v}={n}" for v, n in counts.items() if n)
    print(f"pins={len(results)} {summary} exit={code}", file=out)
    return code


def _display(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Check GitHub Actions `uses:` pins against live upstream refs.",
    )
    parser.add_argument("files", nargs="*", type=Path, help="workflow files (default: ci.yml + release.yml)")
    parser.add_argument("--refs-json", type=Path, help="offline ref listing (see module docstring)")
    parser.add_argument(
        "--timeout",
        type=float,
        default=LS_REMOTE_TIMEOUT_S,
        help=f"git ls-remote timeout in seconds (default {LS_REMOTE_TIMEOUT_S})",
    )
    args = parser.parse_args(argv)

    files = list(args.files) or list(DEFAULT_FILES)
    for f in files:
        if not f.is_file():
            print(f"error: not a file: {f}", file=sys.stderr)
            return 2
    if args.refs_json is not None:
        try:
            list_refs = load_refs_json(args.refs_json)
        except (OSError, ValueError) as exc:
            print(f"error: cannot load --refs-json: {exc}", file=sys.stderr)
            return 2
    else:
        timeout = args.timeout

        def list_refs(owner_repo: str) -> list[RefPair]:
            return git_list_refs(owner_repo, timeout=timeout)

    return run(files, list_refs)


if __name__ == "__main__":
    sys.exit(main())
