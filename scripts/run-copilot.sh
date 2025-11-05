#!/bin/bash

# shellcheck disable=SC2155
export XDG_CONFIG_HOME="$(pwd)/.copilot"
copilot --allow-all-tools --allow-all-paths "$@"
