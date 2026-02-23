#!/usr/bin/env bash
# Tests for the "Extract issue title" logic in bugbot-to-issue.yml
#
# This script exercises the same pipeline used in the workflow:
#   title=$(printf '%s' "$BODY" | grep -m1 -v '^[[:space:]]*$' \
#           | sed 's/^#*[[:space:]]*//' | cut -c1-80) || true
#   echo "title=${title:-BugBot detected an issue}"
#
# Usage: bash .github/workflows/tests/test_extract_title.sh

set -euo pipefail

PASS=0
FAIL=0

assert_title() {
  local test_name="$1"
  local body="$2"
  local expected="$3"

  # Replicate the exact pipeline from the workflow (with || true fix)
  local title
  title=$(printf '%s' "$body" | grep -m1 -v '^[[:space:]]*$' | sed 's/^#*[[:space:]]*//' | cut -c1-80) || true
  local result="${title:-BugBot detected an issue}"

  if [[ "$result" == "$expected" ]]; then
    echo "PASS: $test_name"
    PASS=$((PASS + 1))
  else
    echo "FAIL: $test_name"
    echo "  expected: '$expected'"
    echo "  got:      '$result'"
    FAIL=$((FAIL + 1))
  fi
}

# --- Normal cases ---

assert_title \
  "Normal single-line body" \
  "Title extraction fails on empty comment" \
  "Title extraction fails on empty comment"

assert_title \
  "Body with leading blank line" \
  "
Actual title here" \
  "Actual title here"

assert_title \
  "Body with markdown heading prefix" \
  "### Title extraction fails on empty comment" \
  "Title extraction fails on empty comment"

assert_title \
  "Body with multiple heading hashes" \
  "## Some heading" \
  "Some heading"

# --- Edge cases (the bug scenario) ---

assert_title \
  "Empty body" \
  "" \
  "BugBot detected an issue"

assert_title \
  "Whitespace-only body (spaces)" \
  "   " \
  "BugBot detected an issue"

assert_title \
  "Whitespace-only body (tabs)" \
  "		" \
  "BugBot detected an issue"

assert_title \
  "Body with only blank lines" \
  "

" \
  "BugBot detected an issue"

# --- Truncation ---

long_title="$(printf 'A%.0s' {1..120})"
expected_truncated="$(printf 'A%.0s' {1..80})"
assert_title \
  "Long title truncated to 80 chars" \
  "$long_title" \
  "$expected_truncated"

# --- Summary ---

echo ""
echo "Results: $PASS passed, $FAIL failed"

if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
