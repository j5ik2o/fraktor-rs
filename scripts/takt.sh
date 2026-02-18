#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

usage() {
  cat <<'EOF'
使い方: scripts/takt.sh [オプション] [-- takt引数...]

takt を実行するラッパースクリプト。

オプション:
  --piece <path>      ピース YAML のパス (takt -w に渡す)
  --task <text>       タスク内容 (takt -t に渡す)
  --dry-run           実行コマンドを表示するのみ
  -h, --help          このヘルプを表示

例:
  scripts/takt.sh --piece .takt/pieces/stub-elimination.yaml --task "グループAを実装"
  scripts/takt.sh --piece .takt/pieces/streams-phase2.yaml -- --quiet
EOF
}

PIECE=""
TASK=""
DRY_RUN=false
TAKT_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --piece)
      PIECE="$2"
      shift 2
      ;;
    --task)
      TASK="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      TAKT_ARGS+=("$@")
      break
      ;;
    *)
      TAKT_ARGS+=("$1")
      shift
      ;;
  esac
done

if ! command -v takt &>/dev/null; then
  echo "エラー: takt が見つかりません。" >&2
  exit 1
fi

CMD=(takt)

if [[ -n "${PIECE}" ]]; then
  CMD+=(-w "${PIECE}")
fi

if [[ -n "${TASK}" ]]; then
  CMD+=(-t "${TASK}")
fi

CMD+=("${TAKT_ARGS[@]}")

if [[ "${DRY_RUN}" == true ]]; then
  echo "=== dry-run ===" >&2
  echo "コマンド: ${CMD[*]}" >&2
  exit 0
fi

exec "${CMD[@]}"
