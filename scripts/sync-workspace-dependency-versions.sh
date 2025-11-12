#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <new-version>" >&2
  exit 1
fi

VERSION="$1"

if [[ ! -f Cargo.toml ]]; then
  echo "Error: Cargo.toml not found in current directory" >&2
  exit 1
fi

python3 - "$VERSION" <<'PY'
import pathlib
import re
import sys

version = sys.argv[1]
cargo_path = pathlib.Path("Cargo.toml")
text = cargo_path.read_text()

deps = [
    'fraktor-utils-core-rs',
    'fraktor-utils-std-rs',
    'fraktor-actor-core-rs',
    'fraktor-actor-std-rs',
]

for dep in deps:
    pattern = rf'({dep}\s*=\s*\{{[^}}]*?version\s*=\s*")([^\"]+)("[^}}]*\}})'
    new_text, count = re.subn(pattern, rf'\1{version}\3', text, count=1)
    if count == 0:
        raise SystemExit(f"Failed to update version for {dep} in Cargo.toml")
    text = new_text

cargo_path.write_text(text)
PY

echo "Updated workspace dependency versions to ${VERSION}"
