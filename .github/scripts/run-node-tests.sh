#!/usr/bin/env bash
set -euo pipefail

shopt -s nullglob
tests=(.github/scripts/*.test.mjs scripts/*.test.mjs)
if (( ${#tests[@]} == 0 )); then
  echo "No Node test files found"
  exit 1
fi
node --test --test-reporter=spec "${tests[@]}"
