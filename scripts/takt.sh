#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

# デフォルト値
DEFAULT_BASE_URL="http://127.0.0.1:8317"
DEFAULT_MODEL="gpt-5.3-codex"

usage() {
  cat <<'EOF'
使い方: scripts/takt.sh [オプション] [-- takt引数...]

CLI Proxy API 経由で takt を実行するラッパースクリプト。

オプション:
  --base-url <url>    CLI Proxy API の URL [デフォルト: http://127.0.0.1:8317]
  --model <model>     使用モデル [デフォルト: gpt-5.3-codex]
  --piece <path>      ピース YAML のパス (takt -w に渡す)
  --no-proxy          CLI Proxy API を使わず通常の takt として実行
  --dry-run           実行コマンドを表示するのみ
  -h, --help          このヘルプを表示

環境変数:
  CLI_PROXY_API_BASE_URL  プロキシの URL (--base-url より優先度低)
  CLI_PROXY_API_KEY       プロキシの認証キー (必須、--no-proxy 時は不要)
  TAKT_MODEL              デフォルトモデル (--model より優先度低)

例:
  # Phase 2 の distinct を実装
  scripts/takt.sh --piece .takt/pieces/streams-phase2.yaml "distinct/distinctByを実装"

  # モデル指定
  scripts/takt.sh --model claude-sonnet-4-5-20250929 --piece .takt/pieces/streams-phase2.yaml "..."

  # takt に直接オプションを渡す (-- の後)
  scripts/takt.sh --piece .takt/pieces/streams-phase2.yaml -- --quiet "..."

  # プロキシなしで通常実行
  scripts/takt.sh --no-proxy --piece .takt/pieces/streams-phase2.yaml "..."
EOF
}

# 引数パース
BASE_URL="${CLI_PROXY_API_BASE_URL:-${DEFAULT_BASE_URL}}"
MODEL="${TAKT_MODEL:-${DEFAULT_MODEL}}"
PIECE=""
NO_PROXY=false
DRY_RUN=false
TAKT_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base-url)
      BASE_URL="$2"
      shift 2
      ;;
    --model)
      MODEL="$2"
      shift 2
      ;;
    --piece)
      PIECE="$2"
      shift 2
      ;;
    --no-proxy)
      NO_PROXY=true
      shift
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

# takt コマンドの存在確認
if ! command -v takt &>/dev/null; then
  echo "エラー: takt が見つかりません。npm install -g @anthropic-ai/takt でインストールしてください。" >&2
  exit 1
fi

# 環境変数の設定
if [[ "${NO_PROXY}" == false ]]; then
  if [[ -z "${CLI_PROXY_API_KEY:-}" ]]; then
    echo "エラー: CLI_PROXY_API_KEY が設定されていません。" >&2
    echo "  export CLI_PROXY_API_KEY=<your-key>" >&2
    echo "  または --no-proxy でプロキシなし実行してください。" >&2
    exit 1
  fi

  export ANTHROPIC_BASE_URL="${BASE_URL}"
  export TAKT_ANTHROPIC_API_KEY="${CLI_PROXY_API_KEY}"
fi

# takt コマンドの組み立て
CMD=(takt)

if [[ -n "${PIECE}" ]]; then
  CMD+=(-w "${PIECE}")
fi

CMD+=(--model "${MODEL}")
CMD+=("${TAKT_ARGS[@]}")

# 実行
if [[ "${DRY_RUN}" == true ]]; then
  echo "=== dry-run ===" >&2
  echo "ANTHROPIC_BASE_URL=${ANTHROPIC_BASE_URL:-<未設定>}" >&2
  echo "TAKT_ANTHROPIC_API_KEY=${TAKT_ANTHROPIC_API_KEY:+<設定済み>}" >&2
  echo "コマンド: ${CMD[*]}" >&2
  exit 0
fi

echo "takt 実行: model=${MODEL}, proxy=${NO_PROXY:+無効}${NO_PROXY:-有効}" >&2
if [[ "${NO_PROXY}" == false ]]; then
  echo "  base_url=${BASE_URL}" >&2
fi

exec "${CMD[@]}"
