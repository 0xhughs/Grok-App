#!/usr/bin/env bash
# Loop identity helper (coordinator + reviewer). Read-only; never mutates inputs.
#
#   identity.sh candidate [REPO]   -> candidate snapshot identity
#   identity.sh contract  [REPO]   -> contract identity
#   identity.sh both      [REPO]
#
# Candidate identity = sha256 over `git ls-tree -r HEAD` (mode, type, blob, path)
# with the pack directory `grokbuild-followup-project-loop/` excluded, so
# state-only protocol writes never change candidate identity. Requires a clean
# tree (`git status --porcelain=v1` empty, excluding the pack dir); otherwise a
# SHA-256 manifest of every tracked+untracked non-excluded path is emitted instead
# and its digest is the identity.
#
# Exclusions (both modes): target/, node_modules/, dist/, src-tauri/target/,
# grokbuild-followup-project-loop/ (protocol files + artifacts).
#
# Contract identity = sha256 over AGENTS.md, LOOP.md, BUILDER.md, REVIEWER.md,
# this script, SLICES.md minus mutable sections (Run status, Release evidence,
# Shipped), and BUILD.md from the top through the end of `## Tests`.
set -euo pipefail
MODE="${1:-both}"
REPO="${2:-$(cd "$(dirname "$0")/../.." && pwd)}"
PACK="$REPO/grokbuild-followup-project-loop"

candidate() {
  local dirty
  dirty="$(git -C "$REPO" status --porcelain=v1 -- . ':!grokbuild-followup-project-loop' | grep -v -E '^(\?\?|.. )?(target/|node_modules/|dist/|src-tauri/target/)' || true)"
  echo "HEAD=$(git -C "$REPO" rev-parse HEAD)"
  if [ -z "$dirty" ]; then
    echo "MODE=clean-tree"
    echo "CANDIDATE=$(git -C "$REPO" ls-tree -r HEAD | grep -v -P '\tgrokbuild-followup-project-loop/' | sha256sum | cut -d' ' -f1)"
  else
    echo "MODE=dirty-manifest"
    local manifest
    manifest="$(
      cd "$REPO" && git ls-files -z --cached --others --exclude-standard \
        -- . ':!grokbuild-followup-project-loop' ':!target' ':!node_modules' ':!dist' ':!src-tauri/target' \
        | sort -z | while IFS= read -r -d '' f; do
            if [ -L "$f" ]; then printf 'L %s -> %s\n' "$f" "$(readlink "$f")";
            elif [ -f "$f" ]; then printf '%s %s %s\n' "$(stat -c %a "$f")" "$(sha256sum "$f" | cut -d' ' -f1)" "$f";
            fi
          done
    )"
    echo "CANDIDATE=$(printf '%s\n' "$manifest" | sha256sum | cut -d' ' -f1)"
  fi
}

contract() {
  {
    cat "$PACK/AGENTS.md" "$PACK/LOOP.md" "$PACK/BUILDER.md" "$PACK/REVIEWER.md" "$PACK/artifacts/identity.sh"
    awk '/^## (Run status|Release evidence|Shipped)$/{skip=1;next} /^## /{skip=0} !skip' "$PACK/SLICES.md"
    awk '/^## Proof$/{exit} 1' "$PACK/BUILD.md"
  } | sha256sum | cut -d' ' -f1
}

case "$MODE" in
  candidate) candidate ;;
  contract) echo "CONTRACT=$(contract)" ;;
  both) candidate; echo "CONTRACT=$(contract)" ;;
  *) echo "usage: identity.sh candidate|contract|both [REPO]" >&2; exit 2 ;;
esac
