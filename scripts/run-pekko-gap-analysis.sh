#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)

# ACCOUNT environment variable takes precedence over first positional argument.
if [[ -z "${ACCOUNT:-}" ]]; then
  ACCOUNT="${1:-personal}"
  shift 2>/dev/null || true
fi

# Short aliases
case "$ACCOUNT" in
  p) ACCOUNT=personal ;;
  c) ACCOUNT=corporate ;;
  z) ACCOUNT=zai ;;
esac

CLAUDE_WRAPPER="${SCRIPT_DIR}/run-claude-${ACCOUNT}.sh"
CODEX_WRAPPER="${SCRIPT_DIR}/run-codex-${ACCOUNT}.sh"

if [[ ! -f "$CLAUDE_WRAPPER" && ! -f "$CODEX_WRAPPER" ]]; then
  echo "[ERROR] Unknown account: $ACCOUNT" >&2
  echo "[INFO] Available: $(ls "$SCRIPT_DIR"/run-claude-*.sh 2>/dev/null | sed 's/.*run-claude-//;s/\.sh//' | paste -sd', ')" >&2
  echo "[INFO] Short aliases: p=personal, c=corporate, z=zai" >&2
  exit 1
fi

[[ -f "$CLAUDE_WRAPPER" ]] && export TAKT_CLAUDE_CLI_PATH="$CLAUDE_WRAPPER"
[[ -f "$CODEX_WRAPPER" ]] && export TAKT_CODEX_CLI_PATH="$CODEX_WRAPPER"

MODULES=("actor" "streams" "remote" "cluster" "persistence")
LOG_DIR="${SCRIPT_DIR}/../.takt/logs"
mkdir -p "$LOG_DIR"

pids=()
for mod in "${MODULES[@]}"; do
  log_file="${LOG_DIR}/pekko-gap-analysis-${mod}-${ACCOUNT}.log"
  echo "[INFO] Starting pekko-gap-analysis for module: $mod (account: $ACCOUNT, log: $log_file)"
  npm run takt -- -w pekko-gap-analysis -t "$mod" >"$log_file" 2>&1 &
  pids+=($!)
done

# 全プロセスの完了を待機
failed=0
for pid in "${pids[@]}"; do
  if ! wait "$pid"; then
    failed=$((failed + 1))
  fi
done

if [ "$failed" -gt 0 ]; then
  echo "[WARN] $failed module(s) failed. Check logs in $LOG_DIR"
  exit 1
else
  echo "[INFO] All modules completed successfully"
fi
