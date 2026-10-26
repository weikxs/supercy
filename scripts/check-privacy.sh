#!/usr/bin/env bash
# Fails when tracked or about-to-be-tracked files contain local paths, secrets,
# or any identifier listed in the git-ignored .privacy-denylist file. Docs,
# examples and tests use demo data only (docs/development/standards.md, section 4).
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

readonly DENYLIST=".privacy-denylist"
readonly SELF="scripts/check-privacy.sh"

# Real home directories. Tests use /tmp/supercy-test instead.
# shellcheck disable=SC1003 # the Windows pattern ends in a literal backslash, not an escaped quote
home_patterns=(
  '(^|[^A-Za-z0-9._/-])/Users/[A-Za-z0-9._-]+/'    # macOS home directories
  '(^|[^A-Za-z0-9._/-])/home/[a-z_][a-z0-9._-]*/'  # Linux home directories
  '[A-Za-z]:\\Users\\'                              # Windows home directories
)

# Generic patterns that are never acceptable in this repository.
patterns=(
  '-----BEGIN [A-Z ]*PRIVATE KEY-----'             # private keys
  'gh[pousr]_[A-Za-z0-9]{36}'                       # GitHub tokens
  'github_pat_[A-Za-z0-9_]{22,}'                    # GitHub fine-grained tokens
  'AKIA[0-9A-Z]{16}'                                # AWS access keys
  'xox[abpr]-[A-Za-z0-9-]{10,}'                     # Slack tokens
  'sk-[A-Za-z0-9_-]{32,}'                           # API secret keys of the sk- form
)

# Real identifiers (names, accounts, companies) live only in the local denylist.
if [[ -f "$DENYLIST" ]]; then
  while IFS= read -r line; do
    [[ -z "$line" || "$line" == \#* ]] && continue
    patterns+=("$line")
  done <"$DENYLIST"
fi

# search prints file:line:text for every match. --untracked covers new files;
# ignored files (docs/prd, the denylist, build output) are skipped.
search() {
  local flags=$1
  shift
  local args=()
  for p in "$@"; do
    args+=(-e "$p")
  done
  # shellcheck disable=SC2086 # $flags is empty or one option; empty must vanish
  git grep --untracked -n -I $flags -E "${args[@]}" -- . ":(exclude)$SELF" ':(exclude)*.lock' 2>/dev/null || true
}

matches=$(
  search -i "${patterns[@]}"
  search -i "${home_patterns[@]}"
)
matches=$(sed '/^$/d' <<<"$matches")

if [[ -n "$matches" ]]; then
  echo "privacy check failed: remove local paths, secrets or real identifiers from:" >&2
  # Print locations only, never the matched text, so secrets are not echoed.
  cut -d: -f1,2 <<<"$matches" | sort -u >&2
  exit 1
fi

echo "privacy check passed"
