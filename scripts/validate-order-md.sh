#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

usage() {
  cat <<'EOF'
使い方: scripts/validate-order-md.sh [order.md ...]

引数未指定時:
  .takt/tasks/**/order.md をすべて検証します。

引数指定時:
  指定した order.md ファイルのみ検証します（複数可）。

検証ルール:
  - 先頭の非空行が `# タスク仕様`
  - 次の見出しが存在: `## 目的`, `## 要件`, `## 受け入れ基準`, `## 参考情報`
  - 各セクションに本文がある（空でない）
  - `## 要件` にはチェックボックス行（`- [ ] ...` など）が1件以上ある
EOF
}

log_ok() {
  printf '[OK] %s\n' "$1"
}

log_ng() {
  printf '[NG] %s\n' "$1" >&2
}

extract_section_body() {
  local file="$1"
  local header="$2"
  awk -v header="${header}" '
    $0 == header { in_section = 1; next }
    in_section && /^## / { exit }
    in_section { print }
  ' "${file}"
}

has_non_empty_line() {
  local text="$1"
  [[ -n "$(printf '%s\n' "${text}" | grep -E '[^[:space:]]' || true)" ]]
}

validate_file() {
  local file="$1"
  local failed=0

  if [[ ! -f "${file}" ]]; then
    log_ng "${file}: ファイルが存在しません"
    return 1
  fi

  local first_non_empty
  first_non_empty="$(grep -m1 -v '^[[:space:]]*$' "${file}" || true)"
  if [[ "${first_non_empty}" != "# タスク仕様" ]]; then
    log_ng "${file}: 先頭の非空行は '# タスク仕様' である必要があります"
    failed=1
  fi

  local -a required_headers=(
    "## 目的"
    "## 要件"
    "## 受け入れ基準"
    "## 参考情報"
  )

  local header
  for header in "${required_headers[@]}"; do
    if ! grep -Fqx "${header}" "${file}"; then
      log_ng "${file}: 見出し '${header}' がありません"
      failed=1
      continue
    fi

    local body
    body="$(extract_section_body "${file}" "${header}")"
    if ! has_non_empty_line "${body}"; then
      log_ng "${file}: セクション '${header}' の本文が空です"
      failed=1
    fi
  done

  if grep -Fqx "## 要件" "${file}"; then
    local requirements_body
    requirements_body="$(extract_section_body "${file}" "## 要件")"
    if ! printf '%s\n' "${requirements_body}" | grep -Eq '^[[:space:]]*-[[:space:]]*\[[ xX]\][[:space:]]+'; then
      log_ng "${file}: '## 要件' にチェックボックス行（- [ ] ...）がありません"
      failed=1
    fi
  fi

  if [[ "${failed}" -eq 0 ]]; then
    log_ok "${file}"
    return 0
  fi
  return 1
}

main() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    usage
    return 0
  fi

  local -a targets=()
  if [[ "$#" -gt 0 ]]; then
    targets=("$@")
  else
    while IFS= read -r file; do
      targets+=("${file}")
    done < <(find .takt/tasks -type f -name 'order.md' | sort)
  fi

  if [[ "${#targets[@]}" -eq 0 ]]; then
    echo "検証対象の order.md が見つかりませんでした。"
    return 0
  fi

  local total=0
  local errors=0
  local target
  for target in "${targets[@]}"; do
    total=$((total + 1))
    if ! validate_file "${target}"; then
      errors=$((errors + 1))
    fi
  done

  if [[ "${errors}" -gt 0 ]]; then
    echo "検証失敗: ${errors}/${total} 件" >&2
    return 1
  fi

  echo "検証成功: ${total} 件"
  return 0
}

main "$@"
