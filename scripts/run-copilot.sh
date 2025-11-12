#!/bin/bash

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "${SCRIPT_DIR}/.." && pwd)

# shellcheck disable=SC2155
export XDG_CONFIG_HOME="${REPO_ROOT}/.copilot"
copilot --allow-all-tools --allow-all-paths "$@"
