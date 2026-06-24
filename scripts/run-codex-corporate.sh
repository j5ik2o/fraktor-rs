#!/usr/bin/env bash

export PATH="${HOME}/.agents/bin:$PATH"
export CODEX_HOME="${HOME}/.codex"

exec mise exec -- ${HOME}/.agents/bin/codex --dangerously-bypass-approvals-and-sandbox "$@"
