#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

THUMB_TARGETS=("thumbv6m-none-eabi" "thumbv8m.main-none-eabi")
declare -a HARDWARE_PACKAGES=()
declare -a PARALLEL_PIDS=()
declare -a PARALLEL_LABELS=()

resolve_pinned_toolchain() {
  if [[ -f "${REPO_ROOT}/rust-toolchain.toml" ]]; then
    local channel=""
    channel=$(awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/ {print $2; exit}' "${REPO_ROOT}/rust-toolchain.toml")
    if [[ -n "${channel}" ]]; then
      echo "${channel}"
      return 0
    fi
  fi

  echo "nightly"
}

resolve_python3_bin() {
  if [[ -x "/usr/bin/python3" ]]; then
    printf '%s\n' "/usr/bin/python3"
    return 0
  fi

  local python_bin=""
  python_bin="$(command -v python3 || true)"
  if [[ -z "${python_bin}" ]]; then
    echo "エラー: python3 バイナリを特定できませんでした。" >&2
    return 1
  fi

  printf '%s\n' "${python_bin}"
}

PINNED_TOOLCHAIN="$(resolve_pinned_toolchain)"
DEFAULT_TOOLCHAIN="${PINNED_TOOLCHAIN}"
if [[ -n "${RUSTUP_TOOLCHAIN:-}" && "${RUSTUP_TOOLCHAIN}" != "${PINNED_TOOLCHAIN}" ]]; then
  echo "info: RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN} を上書きして ${PINNED_TOOLCHAIN} を使用します" >&2
fi
export RUSTUP_TOOLCHAIN="${PINNED_TOOLCHAIN}"
FMT_TOOLCHAIN="${FMT_TOOLCHAIN:-${PINNED_TOOLCHAIN}}"

# cargo の呼び出し prefix を確定する。
# `cargo +<toolchain>` は `cargo` が rustup の proxy (~/.cargo/bin/cargo) で
# あることを前提とした構文だが、PATH 順序によっては toolchain 直下の
# cargo バイナリ (例: ~/.rustup/toolchains/<toolchain>/bin/cargo) が先に
# 解決されることがあり、その場合 `+<toolchain>` を解釈できずに
# `error: no such command: '+<toolchain>'` で失敗する。
# rustup が利用可能なら `rustup run <toolchain> cargo` 経由で起動して
# PATH 順序に依存しないようにする。pinned toolchain を使う場合は
# rustup 必須であり、rustup が無い環境では明示的に失敗させる。
_CI_CHECK_HAS_RUSTUP=0
if command -v rustup >/dev/null 2>&1; then
  _CI_CHECK_HAS_RUSTUP=1
fi

build_cargo_prefix_for() {
  local toolchain="${1:-}"
  CI_CHECK_CARGO_PREFIX=()
  if [[ -n "${toolchain}" ]]; then
    if [[ "${_CI_CHECK_HAS_RUSTUP}" == "1" ]]; then
      CI_CHECK_CARGO_PREFIX=(rustup run "${toolchain}" cargo)
    else
      echo "エラー: pinned toolchain (${toolchain}) を使うには rustup が必要です。" >&2
      return 1
    fi
  else
    CI_CHECK_CARGO_PREFIX=(cargo)
  fi
}

build_cargo_prefix_for "${DEFAULT_TOOLCHAIN}" || exit 1
DEFAULT_CARGO_CMD=("${CI_CHECK_CARGO_PREFIX[@]}")
build_cargo_prefix_for "${FMT_TOOLCHAIN}" || exit 1
FMT_CARGO_CMD=("${CI_CHECK_CARGO_PREFIX[@]}")
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
export CARGO_BUILD_JOBS
CI_CHECK_GUARD_TIMEOUT_SEC="${CI_CHECK_GUARD_TIMEOUT_SEC:-0}"
CI_CHECK_GUARD_TIMEOUT_UNIT_SEC="${CI_CHECK_GUARD_TIMEOUT_UNIT_SEC:-}"
CI_CHECK_GUARD_TIMEOUT_INTEGRATION_SEC="${CI_CHECK_GUARD_TIMEOUT_INTEGRATION_SEC:-}"
CI_CHECK_GUARD_KILL_AFTER_SEC="${CI_CHECK_GUARD_KILL_AFTER_SEC:-15}"
CI_CHECK_HANG_COOLDOWN_SEC="${CI_CHECK_HANG_COOLDOWN_SEC:-1800}"
CI_CHECK_HANG_RECORD_FILE="${CI_CHECK_HANG_RECORD_FILE:-${REPO_ROOT}/target/.ci-check.last-hang}"
CI_CHECK_ALLOW_RERUN_AFTER_HANG="${CI_CHECK_ALLOW_RERUN_AFTER_HANG:-0}"
export CI_CHECK_GUARD_TIMEOUT_SEC
export CI_CHECK_GUARD_TIMEOUT_UNIT_SEC
export CI_CHECK_GUARD_TIMEOUT_INTEGRATION_SEC
export CI_CHECK_GUARD_KILL_AFTER_SEC
export CI_CHECK_HANG_COOLDOWN_SEC
export CI_CHECK_HANG_RECORD_FILE
export CI_CHECK_ALLOW_RERUN_AFTER_HANG

usage() {
  cat <<'EOF'
使い方: scripts/ci-check.sh [コマンド...]
  ai [コマンド...]         : AI 向けガード付きで実行します。後続コマンド省略時は all を実行します
  lint                   : cargo fmt --all --check を実行します
  fmt                    : cargo fmt --all を実行します
  dylint [lint...]       : カスタムリントを実行します (デフォルトはすべて、例: dylint mod-file-lint)
                           CSV 形式のショートハンドも利用可能です (例: dylint:mod-file-lint,module-wiring-lint)
  clippy                 : cargo clippy --workspace --all-targets -- -D warnings を実行します
  no-std                 : no_std 対応チェック (core/utils) を実行します
  std                    : std フィーチャーでのテストを実行します
  doc                    : ドキュメントテストを test-support フィーチャー付きで実行します
  examples               : ワークスペース配下の examples をビルドします
  embedded / embassy     : embedded 系 (utils / actor) のチェックとテストを実行します
  unit-test              : ワークスペースの単体テスト (--lib --bins) を実行します
  integration-test       : ワークスペースの統合テスト (--tests --examples) と cross-crate E2E を実行します
  e2e-test               : cross-crate E2E 専用 crate を実行します
  test                   : unit-test + integration-test を順に実行します
  check-unit-sleep       : unit テストパスに実時間 sleep が残っていないことを検査します
  perf                   : Scheduler ストレスと actor ベンチマークを実行します
  actor-path-e2e         : fraktor-actor-core-rs の actor_path_e2e テストを単体実行します
  all                    : AI 向けの標準フルチェックを順番に実行します (引数なし時と同じ)
複数指定で部分実行が可能です (例: scripts/ci-check.sh lint dylint module-wiring-lint)

環境変数:
  CARGO_BUILD_JOBS            : cargo の並列ジョブ数（未設定時は 4）
  CI_CHECK_GUARD_TIMEOUT_SEC  : cargo test/run/bench/nextest の実行上限秒数（0 で無効、既定 0）
  CI_CHECK_GUARD_TIMEOUT_UNIT_SEC : unit-test 用の実行上限秒数（未設定時は CI_CHECK_GUARD_TIMEOUT_SEC を使用）
  CI_CHECK_GUARD_TIMEOUT_INTEGRATION_SEC : integration-test 用の実行上限秒数（未設定時は CI_CHECK_GUARD_TIMEOUT_SEC を使用）
  CI_CHECK_GUARD_KILL_AFTER_SEC : タイムアウト後に強制終了へ移るまでの猶予秒数（既定 15）
  CI_CHECK_HANG_COOLDOWN_SEC  : HANG_SUSPECT 後に同一コマンドの再実行を拒否する秒数（既定 1800）
  CI_CHECK_ALLOW_RERUN_AFTER_HANG : 1 のとき HANG_SUSPECT 後の同一コマンド再実行を許可
EOF
}

log_step() {
  printf '==> %s\n' "$1"
}

resolve_timeout_command() {
  if command -v timeout >/dev/null 2>&1; then
    echo "timeout"
    return 0
  fi

  if command -v gtimeout >/dev/null 2>&1; then
    echo "gtimeout"
    return 0
  fi

  return 1
}

render_command() {
  local rendered=""
  local arg=""
  for arg in "$@"; do
    if [[ -n "${rendered}" ]]; then
      rendered+=" "
    fi
    rendered+="$(printf '%q' "${arg}")"
  done
  printf '%s\n' "${rendered}"
}

should_guard_cargo_command() {
  local subcommand="${1:-}"
  case "${subcommand}" in
    test|run|bench|nextest)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

record_hang_suspect() {
  local command_string="$1"
  mkdir -p "$(dirname "${CI_CHECK_HANG_RECORD_FILE}")"
  printf '%s\t%s\n' "$(date +%s)" "${command_string}" > "${CI_CHECK_HANG_RECORD_FILE}"
}

clear_hang_suspect() {
  rm -f "${CI_CHECK_HANG_RECORD_FILE}" >/dev/null 2>&1 || true
}

guard_against_repeat_hang() {
  local command_string="$1"

  if [[ "${CI_CHECK_ALLOW_RERUN_AFTER_HANG}" == "1" ]]; then
    return 0
  fi

  if [[ ! -f "${CI_CHECK_HANG_RECORD_FILE}" ]]; then
    return 0
  fi

  local recorded_at=""
  local recorded_command=""
  IFS=$'\t' read -r recorded_at recorded_command < "${CI_CHECK_HANG_RECORD_FILE}" || true

  if [[ -z "${recorded_at}" || -z "${recorded_command}" ]]; then
    clear_hang_suspect
    return 0
  fi

  local now
  now=$(date +%s)
  local age=$(( now - recorded_at ))

  if [[ "${recorded_command}" == "${command_string}" && "${age}" -lt "${CI_CHECK_HANG_COOLDOWN_SEC}" ]]; then
    echo "error: 直前に HANG_SUSPECT となった同一コマンドの再実行を拒否しました。" >&2
    echo "error: command=${command_string}" >&2
    echo "error: ${age}s 前に停止しています。コード・対象範囲・仮説・計測を変えるか、明示的に再実行する場合は CI_CHECK_ALLOW_RERUN_AFTER_HANG=1 を指定してください。" >&2
    return 1
  fi

  if [[ "${age}" -ge "${CI_CHECK_HANG_COOLDOWN_SEC}" ]]; then
    clear_hang_suspect
  fi
}

enable_ai_mode() {
  export CI_CHECK_HEARTBEAT_INTERVAL_SEC="${CI_CHECK_HEARTBEAT_INTERVAL_SEC:-30}"
  # Unconditionally set: the top-level default initialises the variable to 0,
  # so the ${..:-1800} expansion never fires.  Override explicitly for AI runs.
  if [[ "${CI_CHECK_GUARD_TIMEOUT_SEC}" == "0" ]]; then
    CI_CHECK_GUARD_TIMEOUT_SEC=1800
  fi
  if [[ -z "${CI_CHECK_GUARD_TIMEOUT_UNIT_SEC}" ]]; then
    CI_CHECK_GUARD_TIMEOUT_UNIT_SEC="${CI_CHECK_GUARD_TIMEOUT_SEC}"
  fi
  if [[ -z "${CI_CHECK_GUARD_TIMEOUT_INTEGRATION_SEC}" ]]; then
    CI_CHECK_GUARD_TIMEOUT_INTEGRATION_SEC=5400
  fi
  export CI_CHECK_GUARD_TIMEOUT_SEC
  export CI_CHECK_GUARD_TIMEOUT_UNIT_SEC
  export CI_CHECK_GUARD_TIMEOUT_INTEGRATION_SEC
  export CI_CHECK_GUARD_KILL_AFTER_SEC="${CI_CHECK_GUARD_KILL_AFTER_SEC:-15}"
  export CI_CHECK_HANG_COOLDOWN_SEC="${CI_CHECK_HANG_COOLDOWN_SEC:-1800}"

  echo "info: AI モードを有効化しました (default-timeout=${CI_CHECK_GUARD_TIMEOUT_SEC}s, unit-timeout=${CI_CHECK_GUARD_TIMEOUT_UNIT_SEC}s, integration-timeout=${CI_CHECK_GUARD_TIMEOUT_INTEGRATION_SEC}s, cooldown=${CI_CHECK_HANG_COOLDOWN_SEC}s, heartbeat=${CI_CHECK_HEARTBEAT_INTERVAL_SEC}s)" >&2
}

run_with_heartbeat() {
  local label="$1"
  shift

  local interval="${CI_CHECK_HEARTBEAT_INTERVAL_SEC:-60}"
  local enabled="${CI_CHECK_HEARTBEAT:-1}"
  local heartbeat_pid=""

  if [[ ! "${interval}" =~ ^[0-9]+$ ]] || [[ "${interval}" -lt 1 ]]; then
    interval=60
  fi

  if [[ "${enabled}" != "0" ]]; then
    local start_time
    start_time=$(date +%s)
    (
      while true; do
        sleep "${interval}"
        local elapsed=$(( $(date +%s) - start_time ))
        printf 'info: %s still running (%ds elapsed)\n' "${label}" "${elapsed}"
      done
    ) &
    heartbeat_pid=$!
  fi

  set +e
  "$@"
  local status=$?
  set -e

  if [[ -n "${heartbeat_pid}" ]]; then
    kill "${heartbeat_pid}" >/dev/null 2>&1 || true
    wait "${heartbeat_pid}" 2>/dev/null || true
  fi

  return "${status}"
}

clean_stale_lint_targets() {
  local lint_path=""
  local removed=0
  for lint_path in "${REPO_ROOT}"/lints/*-lint; do
    if [[ ! -d "${lint_path}" || ! -d "${lint_path}/target" ]]; then
      continue
    fi

    local stale=""
    while IFS= read -r output_file; do
      [[ -n "${output_file}" ]] || continue
      if ! grep -q "${REPO_ROOT}" "${output_file}" 2>/dev/null; then
        stale="yes"
        break
      fi
    done < <(find "${lint_path}/target" -path "*/libssh2-sys-*/output" -type f 2>/dev/null)

    if [[ -n "${stale}" ]]; then
      rm -rf "${lint_path}/target"
      echo "info: ${lint_path#${REPO_ROOT}/}/target を削除しました (libssh2-sys キャッシュの再生成)" >&2
      removed=1
    fi
  done

  if [[ ${removed} -eq 1 ]]; then
    echo "info: Dylint 用ターゲットを再ビルドします" >&2
  fi
}

run_cargo() {
  local -a cmd
  cmd=("${DEFAULT_CARGO_CMD[@]}" -v "$@")

  local command_string=""
  command_string="$(render_command "${cmd[@]}")"
  local guarded="0"

  if should_guard_cargo_command "$1"; then
    guarded="1"
    guard_against_repeat_hang "${command_string}" || return 1

    if [[ "${CI_CHECK_GUARD_TIMEOUT_SEC}" =~ ^[0-9]+$ ]] && [[ "${CI_CHECK_GUARD_TIMEOUT_SEC}" -gt 0 ]]; then
      local timeout_command=""
      timeout_command="$(resolve_timeout_command || true)"
      if [[ -n "${timeout_command}" ]]; then
        set +e
        "${timeout_command}" --foreground --kill-after="${CI_CHECK_GUARD_KILL_AFTER_SEC}" "${CI_CHECK_GUARD_TIMEOUT_SEC}" "${cmd[@]}"
        local status=$?
        set -e
        if [[ "${status}" -ne 0 ]]; then
          if [[ "${status}" -eq 124 ]] || [[ "${status}" -eq 137 ]]; then
            record_hang_suspect "${command_string}"
            echo "error: HANG_SUSPECT: ${command_string}" >&2
            echo "error: ${CI_CHECK_GUARD_TIMEOUT_SEC}s を超過したため停止しました（exit ${status}）。盲目的な再実行は禁止です。対象を絞るか計測を追加してください。" >&2
            return "${status}"
          fi
          clear_hang_suspect
          echo "error: ${command_string}" >&2
          return "${status}"
        fi

        clear_hang_suspect
        return 0
      fi

      echo "warning: timeout コマンドが見つからないため、ハングガードなしで実行します。" >&2
    fi
  fi

  set +e
  "${cmd[@]}"
  local status=$?
  set -e
  if [[ "${status}" -ne 0 ]]; then
    echo "error: ${command_string}" >&2
    return "${status}"
  fi

  if [[ "${guarded}" == "1" ]]; then
    clear_hang_suspect
  fi
}

run_cargo_with_timeout_override() {
  local timeout_override="$1"
  shift

  local previous_timeout="${CI_CHECK_GUARD_TIMEOUT_SEC}"
  if [[ -z "${timeout_override}" ]] || ! [[ "${timeout_override}" =~ ^[0-9]+$ ]] || [[ "${timeout_override}" -le 0 ]]; then
    CI_CHECK_GUARD_TIMEOUT_SEC="${previous_timeout}"
    echo "error: run_cargo_with_timeout_override に渡す CI_CHECK_GUARD_TIMEOUT_SEC は 1 以上の整数秒で指定してください: ${timeout_override:-<empty>}" >&2
    return 1
  fi

  CI_CHECK_GUARD_TIMEOUT_SEC="${timeout_override}"

  set +e
  run_cargo "$@"
  local status=$?
  set -e

  CI_CHECK_GUARD_TIMEOUT_SEC="${previous_timeout}"
  return "${status}"
}

start_parallel_cargo() {
  local label="$1"
  local shard="$2"
  shift 2

  local target_dir="${REPO_ROOT}/target/ci-check/${shard}"
  mkdir -p "${target_dir}"

  log_step "[parallel] ${label} (CARGO_TARGET_DIR=${target_dir#${REPO_ROOT}/})"
  (
    CARGO_TARGET_DIR="${target_dir}" run_cargo "$@"
  ) &

  PARALLEL_PIDS+=("$!")
  PARALLEL_LABELS+=("${label}")
}

start_parallel_phase() {
  local label="$1"
  local shard="$2"
  local func="$3"

  local target_dir="${REPO_ROOT}/target/ci-check/${shard}"
  mkdir -p "${target_dir}"

  log_step "[parallel] ${label} (CARGO_TARGET_DIR=${target_dir#${REPO_ROOT}/})"
  (
    export CARGO_TARGET_DIR="${target_dir}"
    "${func}"
  ) &

  PARALLEL_PIDS+=("$!")
  PARALLEL_LABELS+=("${label}")
}

wait_parallel_cargo() {
  local failed=0
  local failed_label=""

  # ポーリングで全ジョブの完了を監視し、最初の失敗で他を kill する
  while true; do
    local still_running=0
    local idx
    for idx in "${!PARALLEL_PIDS[@]}"; do
      local pid="${PARALLEL_PIDS[${idx}]}"
      [[ -n "${pid}" ]] || continue
      if ! kill -0 "${pid}" 2>/dev/null; then
        # プロセス終了済み — 終了コードを回収
        if ! wait "${pid}" 2>/dev/null; then
          echo "error: 並行ジョブ失敗: ${PARALLEL_LABELS[${idx}]}" >&2
          failed=1
          failed_label="${PARALLEL_LABELS[${idx}]}"
          PARALLEL_PIDS[${idx}]=""
        else
          PARALLEL_PIDS[${idx}]=""
        fi
      else
        still_running=1
      fi
    done

    if [[ ${failed} -ne 0 ]]; then
      # 残りのジョブを kill
      for idx in "${!PARALLEL_PIDS[@]}"; do
        local pid="${PARALLEL_PIDS[${idx}]}"
        [[ -n "${pid}" ]] || continue
        kill "${pid}" 2>/dev/null || true
      done
      # zombie 回収
      for idx in "${!PARALLEL_PIDS[@]}"; do
        local pid="${PARALLEL_PIDS[${idx}]}"
        [[ -n "${pid}" ]] || continue
        wait "${pid}" 2>/dev/null || true
      done
      break
    fi

    if [[ ${still_running} -eq 0 ]]; then
      break
    fi
    sleep 0.2
  done

  PARALLEL_PIDS=()
  PARALLEL_LABELS=()

  if [[ ${failed} -ne 0 ]]; then
    return 1
  fi
}

ensure_target_installed() {
  local target="$1"

  if [[ -n "${DEFAULT_TOOLCHAIN}" ]]; then
    if rustup target list --installed --toolchain "${DEFAULT_TOOLCHAIN}" | grep -qx "${target}"; then
      return 0
    fi
  else
    if rustup target list --installed | grep -qx "${target}"; then
      return 0
    fi
  fi

  if [[ -n "${CI:-}" ]]; then
    echo "info: installing target ${target}" >&2
    if [[ -n "${DEFAULT_TOOLCHAIN}" ]]; then
      if rustup target add --toolchain "${DEFAULT_TOOLCHAIN}" "${target}"; then
        return 0
      fi
    else
      if rustup target add "${target}"; then
        return 0
      fi
    fi
    echo "エラー: ターゲット ${target} のインストールに失敗しました。" >&2
    return 1
  fi

  echo "警告: ターゲット ${target} が見つからなかったためクロスチェックをスキップします。" >&2
  return 2
}

get_dylib_extension() {
  case "$(uname -s)" in
    Darwin*)
      echo "dylib"
      ;;
    Linux*)
      echo "so"
      ;;
    CYGWIN*|MINGW*|MSYS*)
      echo "dll"
      ;;
    *)
      echo "so"
      ;;
  esac
}

ensure_rustc_components_installed() {
  local -a required_components=("rustc-dev" "llvm-tools-preview")
  local -a missing_components=()
  local component_list=""

  if [[ -n "${DEFAULT_TOOLCHAIN}" ]]; then
    if ! component_list=$(rustup component list --toolchain "${DEFAULT_TOOLCHAIN}" 2>/dev/null); then
      echo "エラー: rustup component list の取得に失敗しました" >&2
      return 1
    fi
  else
    if ! component_list=$(rustup component list 2>/dev/null); then
      echo "エラー: rustup component list の取得に失敗しました" >&2
      return 1
    fi
  fi

  for component in "${required_components[@]}"; do
    local search_name="${component}"
    case "${component}" in
      llvm-tools-preview) search_name="llvm-tools" ;;
    esac

    if ! grep -Eq "^${search_name}([-._[:alnum:]]+)? \(installed\)$" <<<"${component_list}"; then
      missing_components+=("${component}")
    fi
  done

  if [[ ${#missing_components[@]} -eq 0 ]]; then
    return 0
  fi

  echo "info: 不足しているコンポーネントをインストールします: ${missing_components[*]}" >&2
  local component
  for component in "${missing_components[@]}"; do
    if [[ -n "${DEFAULT_TOOLCHAIN}" ]]; then
      if rustup component add --toolchain "${DEFAULT_TOOLCHAIN}" "${component}"; then
        echo "info: ${component} のインストールが完了しました。" >&2
      else
        echo "エラー: ${component} のインストールに失敗しました。" >&2
        return 1
      fi
    else
      if rustup component add "${component}"; then
        echo "info: ${component} のインストールが完了しました。" >&2
      else
        echo "エラー: ${component} のインストールに失敗しました。" >&2
        return 1
      fi
    fi
  done

  return 0
}

ensure_dylint_installed() {
  local desired_version="${DYLINT_VERSION:-5.0.0}"
  local current_version=""

  if command -v cargo-dylint >/dev/null 2>&1; then
    current_version=$(cargo-dylint --version 2>/dev/null | awk '{print $2}')
  fi

  if [[ -z "${current_version}" ]]; then
    current_version=$(cargo dylint --version 2>/dev/null | awk '{print $2}')
  fi

  if [[ "${current_version}" == "${desired_version}" ]]; then
    return 0
  fi

  if [[ -n "${current_version}" ]]; then
    echo "info: cargo-dylint ${current_version:-unknown} を ${desired_version} に更新します..." >&2
  else
    echo "info: cargo-dylint がインストールされていないため、インストールします..." >&2
  fi

  local -a install_cmd
  install_cmd=("${DEFAULT_CARGO_CMD[@]}" -v install cargo-dylint --locked --version "${desired_version}")

  if [[ -n "${current_version}" ]]; then
    install_cmd+=(--force)
  fi

  if "${install_cmd[@]}"; then
    echo "info: cargo-dylint のインストールが完了しました。" >&2
    return 0
  fi

  echo "エラー: cargo-dylint のインストールに失敗しました。" >&2
  echo "手動でインストールする場合: cargo install cargo-dylint" >&2
  return 1
}

run_fmt() {
  local -a fmt_cmd=("${FMT_CARGO_CMD[@]}" -v fmt --all)
  log_step "$(render_command "${fmt_cmd[@]}")"
  "${fmt_cmd[@]}" || return 1
}

run_lint() {
  local -a lint_cmd=("${FMT_CARGO_CMD[@]}" -v fmt --all --check)
  log_step "$(render_command "${lint_cmd[@]}")"
  "${lint_cmd[@]}" || return 1
}

run_dylint() {
  ensure_rustc_components_installed || return 1
  ensure_dylint_installed || return 1

  local -a lint_filters
  lint_filters=()
  local -a module_filters
  module_filters=()
  local -a trailing_args
  trailing_args=()

  while [[ $# -gt 0 ]]; do
    case "$1" in
      -n|--name)
        if [[ $# -lt 2 ]]; then
          echo "エラー: -n/--name にはリント名を指定してください" >&2
          return 1
        fi
        lint_filters+=("$2")
        shift 2
        ;;
      -m|--module|--package)
        if [[ $# -lt 2 ]]; then
          echo "エラー: -m/--module にはモジュール名(またはパッケージ名)を指定してください" >&2
          return 1
        fi
        module_filters+=("$2")
        shift 2
        ;;
      --)
        shift
        while [[ $# -gt 0 ]]; do
          trailing_args+=("$1")
          shift
        done
        break
        ;;
      -h|--help)
        echo "利用例: scripts/ci-check.sh dylint -n mod-file-lint -m fraktor-actor-core-rs" >&2
        return 0
        ;;
      *)
        lint_filters+=("$1")
        shift
        ;;
    esac
  done

  local -a requested=()
  if [[ ${lint_filters+set} == set ]]; then
    requested=("${lint_filters[@]}")
  fi

  local -a lint_entries=(
    "mod-file-lint:lints/mod-file-lint"
    "module-examples-lint:lints/module-examples-lint"
    "module-wiring-lint:lints/module-wiring-lint"
    "type-per-file-lint:lints/type-per-file-lint"
    "tests-location-lint:lints/tests-location-lint"
    "use-placement-lint:lints/use-placement-lint"
    "redundant-fqcn-lint:lints/redundant-fqcn-lint"
    "rustdoc-lint:lints/rustdoc-lint"
    "cfg-std-forbid-lint:lints/cfg-std-forbid-lint"
    "ambiguous-suffix-lint:lints/ambiguous-suffix-lint"
  )

  local -a selected=()

  if [[ ${#requested[@]} -eq 0 ]]; then
    selected=("${lint_entries[@]}")
  else
    local name
    for name in "${requested[@]}"; do
      local found=""
      local entry
      for entry in "${lint_entries[@]}"; do
        local crate="${entry%%:*}"
        local alias="${crate//-/_}"
        if [[ "${name}" == "${crate}" || "${name}" == "${alias}" || "${name}" == "lints/${crate}" || "${name}" == "${entry#*:}" ]]; then
          local already=""
          if [[ ${#selected[@]} -gt 0 ]]; then
            local existing
            for existing in "${selected[@]}"; do
              if [[ "${existing}" == "${entry}" ]]; then
                already="yes"
                break
              fi
            done
          fi
          if [[ -z "${already}" ]]; then
            selected+=("${entry}")
          fi
          found="yes"
          break
        fi
      done
      if [[ -z "${found}" ]]; then
        echo "エラー: 未知のリンター '${name}' が指定されました" >&2
        echo "利用可能: ${lint_entries[*]//:*/}" >&2
        return 1
      fi
    done
  fi

  local -a package_args=()
  if [[ ${#module_filters[@]} -gt 0 ]]; then
    local module_spec
    for module_spec in "${module_filters[@]}"; do
      local manifest_path=""
      local package_name=""

      if [[ -f "${module_spec}" && "${module_spec}" == *Cargo.toml ]]; then
        manifest_path="${module_spec}"
      elif [[ -d "${module_spec}" && -f "${module_spec}/Cargo.toml" ]]; then
        manifest_path="${module_spec}/Cargo.toml"
      elif [[ -d "modules/${module_spec}" && -f "modules/${module_spec}/Cargo.toml" ]]; then
        manifest_path="modules/${module_spec}/Cargo.toml"
      fi

      if [[ -n "${manifest_path}" ]]; then
        package_name="$(awk -F'"' '/^name[[:space:]]*=/{print $2; exit}' "${manifest_path}")"
      else
        package_name="${module_spec}"
      fi

      if [[ -z "${package_name}" ]]; then
        echo "エラー: モジュール '${module_spec}' のパッケージ名を特定できませんでした" >&2
        return 1
      fi

      local already=""
      if [[ ${#package_args[@]} -gt 0 ]]; then
        local idx=0
        while [[ ${idx} -lt ${#package_args[@]} ]]; do
          if [[ "${package_args[${idx}+1]}" == "${package_name}" ]]; then
            already="yes"
            break
          fi
          idx=$((idx + 2))
        done
      fi

      if [[ -z "${already}" ]]; then
        package_args+=("-p" "${package_name}")
      fi
    done
  fi

  local toolchain
  if [[ -f "${REPO_ROOT}/rust-toolchain.toml" ]]; then
     local channel
     channel=$(awk -F'"' '/channel/ {print $2; exit}' "${REPO_ROOT}/rust-toolchain.toml")
     if [[ -n "${channel}" ]]; then
       toolchain="${channel}-$(rustc -vV | awk '/^host:/{print $2}')"
     else
       toolchain="nightly-$(rustc -vV | awk '/^host:/{print $2}')"
     fi
  else
     toolchain="nightly-$(rustc -vV | awk '/^host:/{print $2}')"
  fi
  local -a lib_dirs=()
  local -a dylint_args=()

  local entry
  for entry in "${selected[@]}"; do
    local crate="${entry%%:*}"
    local lint_path="${entry#*:}"
    local lib_name="${crate//-/_}"

    local -a build_cmd=("${DEFAULT_CARGO_CMD[@]}" -v build --manifest-path "${lint_path}/Cargo.toml" --release)
    log_step "$(render_command "${build_cmd[@]}")"
    env -u CARGO_TARGET_DIR CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" "${build_cmd[@]}" || return 1

    local -a test_cmd=("${DEFAULT_CARGO_CMD[@]}" -v test --manifest-path "${lint_path}/Cargo.toml" -- test ui -- --quiet)
    log_step "$(render_command "${test_cmd[@]}")"
    env -u CARGO_TARGET_DIR CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" "${test_cmd[@]}" || return 1

    local dylib_ext
    dylib_ext="$(get_dylib_extension)"
    local target_dir="${lint_path}/target/release"
    local plain_lib="${target_dir}/lib${lib_name}.${dylib_ext}"
    local tagged_lib="${target_dir}/lib${lib_name}@${toolchain}.${dylib_ext}"

    if [[ -f "${plain_lib}" ]]; then
      if compgen -G "${target_dir}/lib${lib_name}@*.${dylib_ext}" >/dev/null; then
        for existing_lib in "${target_dir}"/lib"${lib_name}"@*.${dylib_ext}; do
          [[ "${existing_lib}" == "${tagged_lib}" ]] && continue
          rm -f "${existing_lib}"
        done
      fi

      cp -f "${plain_lib}" "${tagged_lib}"
      lib_dirs+=("$(cd "${target_dir}" && pwd)")
    else
      echo "エラー: ${plain_lib} が見つかりません" >&2
      return 1
    fi

    dylint_args+=("--lib" "${lib_name}")
  done

  local dylint_library_path
  dylint_library_path="$(IFS=:; echo "${lib_dirs[*]}")"

  local rustflags_value
  if [[ -n "${RUSTFLAGS-}" ]]; then
    rustflags_value="${RUSTFLAGS} -Dwarnings --force-warn deprecated"
  else
    rustflags_value="-Dwarnings --force-warn deprecated"
  fi
  local dylint_incremental="${DYLINT_CARGO_INCREMENTAL:-0}"

  local -a common_dylint_args=("${dylint_args[@]}" "--no-metadata")
  local sysroot_lib=""
  sysroot_lib="$(rustc --print sysroot)/lib"
  local dynlib_path="${sysroot_lib}"
  if [[ "$(uname -s)" == "Darwin" ]]; then
    if [[ -n "${DYLD_FALLBACK_LIBRARY_PATH-}" ]]; then
      dynlib_path="${sysroot_lib}:${DYLD_FALLBACK_LIBRARY_PATH}"
    fi
  else
    if [[ -n "${LD_LIBRARY_PATH-}" ]]; then
      dynlib_path="${sysroot_lib}:${LD_LIBRARY_PATH}"
    fi
  fi
  local -a hardware_packages=()
  if [[ ${HARDWARE_PACKAGES+set} == set && ${#HARDWARE_PACKAGES[@]} -gt 0 ]]; then
    hardware_packages=("${HARDWARE_PACKAGES[@]}")
  fi
  local -a main_package_args=()
  local -a hardware_targets=()
  local -a feature_packages=("fraktor-actor-adaptor-std-rs=tokio-executor")

  if [[ ${#package_args[@]} -eq 0 ]]; then
    local python_bin=""
    python_bin="$(resolve_python3_bin)" || return 1
    local metadata_file
    metadata_file="$(mktemp)" || return 1
    if ! env -u CARGO_TARGET_DIR CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" "${DEFAULT_CARGO_CMD[@]}" metadata --format-version 1 --no-deps > "${metadata_file}"; then
      rm -f "${metadata_file}"
      echo "エラー: cargo metadata の取得に失敗しました。" >&2
      return 1
    fi
    local -a python_cmd=("${python_bin}" - "${metadata_file}")
    if [[ ${#hardware_packages[@]} -gt 0 ]]; then
      python_cmd+=("${hardware_packages[@]}")
    fi
    local -a workspace_packages=()
    while IFS= read -r pkg; do
      if [[ -n "${pkg}" ]]; then
        workspace_packages+=("${pkg}")
      fi
    done < <("${python_cmd[@]}" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as f:
    metadata = json.load(f)
hardware = set(sys.argv[2:])
for package in metadata.get("packages", []):
    name = package.get("name")
    if not name or name in hardware:
        continue
    print(name)
PY
    )
    rm -f "${metadata_file}"
    if [[ ${#workspace_packages[@]} -eq 0 ]]; then
      echo "エラー: ワークスペースのパッケージ一覧を取得できませんでした。" >&2
      return 1
    fi
    local pkg
    for pkg in "${workspace_packages[@]}"; do
      main_package_args+=("-p" "${pkg}")
    done
    if [[ ${#hardware_packages[@]} -gt 0 ]]; then
      hardware_targets=("${hardware_packages[@]}")
    fi
  else
    local idx=0
    while [[ ${idx} -lt ${#package_args[@]} ]]; do
      local flag="${package_args[${idx}]}"
      local value="${package_args[${idx}+1]}"
      local matched=""
      if [[ "${flag}" == "-p" ]]; then
        local hpkg
        if [[ ${#hardware_packages[@]} -gt 0 ]]; then
          for hpkg in "${hardware_packages[@]}"; do
            if [[ "${value}" == "${hpkg}" ]]; then
              matched="yes"
              break
            fi
          done
        fi
      fi
      if [[ -n "${matched}" ]]; then
        hardware_targets+=("${value}")
      else
        main_package_args+=("${flag}" "${value}")
      fi
      idx=$((idx + 2))
    done
  fi

  # Remove duplicate hardware targets while preserving order
  if [[ ${#hardware_targets[@]} -gt 1 ]]; then
    local -a deduped=()
    local seen=""
    for pkg in "${hardware_targets[@]}"; do
      if [[ ":${seen}:" != *":${pkg}:"* ]]; then
        deduped+=("${pkg}")
        seen="${seen}:${pkg}"
      fi
    done
    hardware_targets=("${deduped[@]}")
  fi

  local -a main_invocation=()
  if [[ ${#main_package_args[@]} -gt 0 ]]; then
    main_invocation=("${main_package_args[@]}" "${common_dylint_args[@]}")
    local log_main="${main_invocation[*]}"
    local log_trailing=""
    if [[ ${#trailing_args[@]} -gt 0 ]]; then
      log_trailing=" -- ${trailing_args[*]}"
    fi
    log_step "cargo +${DEFAULT_TOOLCHAIN} dylint ${log_main}${log_trailing} (RUSTFLAGS=${rustflags_value}, CARGO_INCREMENTAL=${dylint_incremental})"
    if [[ ${#trailing_args[@]} -gt 0 ]]; then
      RUSTFLAGS="${rustflags_value}" CARGO_INCREMENTAL="${dylint_incremental}" DYLINT_LIBRARY_PATH="${dylint_library_path}" DYLD_FALLBACK_LIBRARY_PATH="${dynlib_path}" LD_LIBRARY_PATH="${dynlib_path}" CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" run_cargo dylint "${main_invocation[@]}" -- "${trailing_args[@]}" || return 1
    else
      RUSTFLAGS="${rustflags_value}" CARGO_INCREMENTAL="${dylint_incremental}" DYLINT_LIBRARY_PATH="${dylint_library_path}" DYLD_FALLBACK_LIBRARY_PATH="${dynlib_path}" LD_LIBRARY_PATH="${dynlib_path}" CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" run_cargo dylint "${main_invocation[@]}" || return 1
    fi
  fi

  # redundant-fqcn-lint 専用の test ターゲット pass:
  # 既定の `cargo dylint` は cargo check の既定ターゲット (lib のみ) で走るため、
  # `#[cfg(test)]` モジュール (兄弟 `tests.rs` を含む) はどの lint からも visit されない。
  # FQCN 整形はテストコードにも適用したいので、redundant-fqcn-lint だけを
  # `--tests` 付きで再走させる。ビルドコストを抑えるためこの lint のみ対象。
  local has_redundant_fqcn=""
  local lint_entry
  for lint_entry in "${selected[@]}"; do
    if [[ "${lint_entry%%:*}" == "redundant-fqcn-lint" ]]; then
      has_redundant_fqcn="yes"
      break
    fi
  done

  if [[ -n "${has_redundant_fqcn}" && ${#main_package_args[@]} -gt 0 ]]; then
    local -a fqcn_tests_invocation=("${main_package_args[@]}" "--lib" "redundant_fqcn_lint" "--no-metadata")
    local -a fqcn_tests_cargo_args=("--tests")
    if [[ ${#trailing_args[@]} -gt 0 ]]; then
      fqcn_tests_cargo_args+=("${trailing_args[@]}")
    fi
    log_step "cargo +${DEFAULT_TOOLCHAIN} dylint ${fqcn_tests_invocation[*]} -- ${fqcn_tests_cargo_args[*]} (redundant-fqcn-lint --tests pass, RUSTFLAGS=${rustflags_value}, CARGO_INCREMENTAL=${dylint_incremental})"
    RUSTFLAGS="${rustflags_value}" CARGO_INCREMENTAL="${dylint_incremental}" DYLINT_LIBRARY_PATH="${dylint_library_path}" DYLD_FALLBACK_LIBRARY_PATH="${dynlib_path}" LD_LIBRARY_PATH="${dynlib_path}" CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" run_cargo dylint "${fqcn_tests_invocation[@]}" -- "${fqcn_tests_cargo_args[@]}" || return 1
  fi

  if [[ ${#hardware_targets[@]} -gt 0 ]]; then
    local pkg
    for pkg in "${hardware_targets[@]}"; do
      local -a pkg_invocation=("-p" "${pkg}" "${common_dylint_args[@]}")
      local log_pkg="${pkg_invocation[*]}"
      local log_trailing=""
      if [[ ${#trailing_args[@]} -gt 0 ]]; then
        log_trailing=" -- ${trailing_args[*]}"
      fi
      log_step "cargo +${DEFAULT_TOOLCHAIN} dylint ${log_pkg}${log_trailing} (RUSTFLAGS=${rustflags_value}, CARGO_INCREMENTAL=${dylint_incremental})"
      if [[ ${#trailing_args[@]} -gt 0 ]]; then
        RUSTFLAGS="${rustflags_value}" CARGO_INCREMENTAL="${dylint_incremental}" DYLINT_LIBRARY_PATH="${dylint_library_path}" DYLD_FALLBACK_LIBRARY_PATH="${dynlib_path}" LD_LIBRARY_PATH="${dynlib_path}" CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" run_cargo dylint "${pkg_invocation[@]}" -- "${trailing_args[@]}" || return 1
      else
        RUSTFLAGS="${rustflags_value}" CARGO_INCREMENTAL="${dylint_incremental}" DYLINT_LIBRARY_PATH="${dylint_library_path}" DYLD_FALLBACK_LIBRARY_PATH="${dynlib_path}" LD_LIBRARY_PATH="${dynlib_path}" CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" run_cargo dylint "${pkg_invocation[@]}" || return 1
      fi
    done
  fi

  if [[ ${#feature_packages[@]} -gt 0 ]]; then
    local feature_mapping
    for feature_mapping in "${feature_packages[@]}"; do
      local feature_pkg="${feature_mapping%%=*}"
      local feature_list="${feature_mapping#*=}"
      local run_feature=""

      if [[ ${#package_args[@]} -eq 0 ]]; then
        run_feature="yes"
      else
        local idx=0
        while [[ ${idx} -lt ${#package_args[@]} ]]; do
          if [[ "${package_args[${idx}]}" == "-p" && "${package_args[${idx}+1]}" == "${feature_pkg}" ]]; then
            run_feature="yes"
            break
          fi
          idx=$((idx + 2))
        done
      fi

      if [[ -z "${run_feature}" ]]; then
        continue
      fi

      local -a feature_invocation=("-p" "${feature_pkg}" "${common_dylint_args[@]}")
      local -a feature_trailing=(--features "${feature_list}")
      local log_feature="${feature_invocation[*]} -- --features ${feature_list}"
      if [[ ${#trailing_args[@]} -gt 0 ]]; then
        log_feature+=" -- ${trailing_args[*]}"
        feature_trailing+=("${trailing_args[@]}")
      fi
      log_step "cargo +${DEFAULT_TOOLCHAIN} dylint ${log_feature} (RUSTFLAGS=${rustflags_value}, CARGO_INCREMENTAL=${dylint_incremental})"
      RUSTFLAGS="${rustflags_value}" CARGO_INCREMENTAL="${dylint_incremental}" DYLINT_LIBRARY_PATH="${dylint_library_path}" DYLD_FALLBACK_LIBRARY_PATH="${dynlib_path}" LD_LIBRARY_PATH="${dynlib_path}" CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" run_cargo dylint "${feature_invocation[@]}" -- "${feature_trailing[@]}" || return 1
    done
  fi
}

run_clippy() {
  # --all-targets は dev-dep 解決時に ahash/proptest のトランジティブ依存が
  # 壊れるため --lib --bins に限定する（テストコードは run_tests で検証される）。
  # postcard 1.1.3 が nightly と非互換のため fraktor-cluster-core-rs / fraktor-cluster-adaptor-std-rs を一時的に除外する。
  log_step "cargo +${DEFAULT_TOOLCHAIN} clippy --workspace --exclude fraktor-cluster-core-rs --exclude fraktor-cluster-adaptor-std-rs --lib --bins -- -D warnings"
  run_cargo clippy --workspace --exclude fraktor-cluster-core-rs --exclude fraktor-cluster-adaptor-std-rs --lib --bins -- -D warnings || return 1
}

run_no_std() {
  PARALLEL_PIDS=()
  PARALLEL_LABELS=()
  start_parallel_cargo \
    "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-utils-core-rs --no-default-features --features alloc" \
    "no-std-host-utils" \
    check -p fraktor-utils-core-rs --no-default-features --features alloc
  start_parallel_cargo \
    "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-core-rs -p fraktor-stream-core-rs -p fraktor-rs --no-default-features" \
    "no-std-host-core" \
    check -p fraktor-actor-core-rs -p fraktor-stream-core-rs -p fraktor-rs --no-default-features
  wait_parallel_cargo || return 1

  local thumb_target="thumbv8m.main-none-eabi"
  if ensure_target_installed "${thumb_target}"; then
    PARALLEL_PIDS=()
    PARALLEL_LABELS=()
    start_parallel_cargo \
      "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-utils-core-rs --no-default-features --target ${thumb_target} -F fraktor-utils-core-rs/alloc" \
      "no-std-thumb-utils" \
      check -p fraktor-utils-core-rs --no-default-features --target "${thumb_target}" -F fraktor-utils-core-rs/alloc
    start_parallel_cargo \
      "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-core-rs -p fraktor-stream-core-rs --no-default-features --target ${thumb_target}" \
      "no-std-thumb-core" \
      check -p fraktor-actor-core-rs -p fraktor-stream-core-rs --no-default-features --target "${thumb_target}"
    wait_parallel_cargo || return 1
  fi
}

run_std() {
  PARALLEL_PIDS=()
  PARALLEL_LABELS=()
  start_parallel_cargo \
    "cargo +${DEFAULT_TOOLCHAIN} test -p fraktor-utils-core-rs" \
    "std-utils" \
    test -p fraktor-utils-core-rs
  start_parallel_cargo \
    "cargo +${DEFAULT_TOOLCHAIN} test -p fraktor-actor-core-rs -p fraktor-stream-core-rs -p fraktor-stream-adaptor-std-rs -p fraktor-rs --lib" \
    "std-core" \
    test -p fraktor-actor-core-rs -p fraktor-stream-core-rs -p fraktor-stream-adaptor-std-rs -p fraktor-rs --lib
  wait_parallel_cargo || return 1
}

run_doc_tests() {
  log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-core-rs --no-default-features"
  run_cargo check -p fraktor-actor-core-rs --no-default-features || return 1
}

# run_embedded() {
#  log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-utils-embedded-rs --no-default-features --features rc"
#  run_cargo check -p fraktor-utils-embedded-rs --no-default-features --features rc || return 1
#
#  log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-utils-embedded-rs --no-default-features --features arc"
#  run_cargo check -p fraktor-utils-embedded-rs --no-default-features --features arc || return 1
#
#  log_step "cargo +${DEFAULT_TOOLCHAIN} test -p fraktor-utils-embedded-rs --no-default-features --features embassy --no-run"
#  run_cargo test -p fraktor-utils-embedded-rs --no-default-features --features embassy --no-run || return 1
#
#  log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-embedded-rs --no-default-features --features alloc,embedded_arc"
#  run_cargo check -p fraktor-actor-embedded-rs --no-default-features --features alloc,embedded_arc || return 1
#
#  log_step "cargo +${DEFAULT_TOOLCHAIN} test -p fraktor-actor-embedded-rs --no-default-features --features alloc,embedded_arc"
#  run_cargo test -p fraktor-actor-embedded-rs --no-default-features --features alloc,embedded_arc || return 1
#
#  for target in "${THUMB_TARGETS[@]}"; do
#    if ! ensure_target_installed "${target}"; then
#      status=$?
#      if [[ ${status} -eq 1 ]]; then
#        return 1
#      fi
#      continue
#    fi
#
#    log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-utils-core-rs --target ${target} --no-default-features --features alloc"
#    run_cargo check -p fraktor-utils-core-rs --target "${target}" --no-default-features --features alloc || return 1
#
#    log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-core-rs --target ${target} --no-default-features --features alloc"
#    run_cargo check -p fraktor-actor-core-rs --target "${target}" --no-default-features --features alloc || return 1
#
#    log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-core-rs --target ${target} --no-default-features --features alloc"
#    run_cargo check -p fraktor-actor-core-rs --target "${target}" --no-default-features --features alloc || return 1
#
#    log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-embedded-rs --target ${target} --no-default-features --features alloc,embedded_rc"
#    run_cargo check -p fraktor-actor-embedded-rs --target "${target}" --no-default-features --features alloc,embedded_rc || return 1
#  done
# }

run_unit_tests() {
  log_step "cargo +${DEFAULT_TOOLCHAIN} test --workspace --verbose --lib --bins --features test-support"
  local timeout_override="${CI_CHECK_GUARD_TIMEOUT_UNIT_SEC:-${CI_CHECK_GUARD_TIMEOUT_SEC}}"
  if [[ "${timeout_override}" == "0" ]]; then
    run_cargo test --workspace --verbose --lib --bins --features test-support || return 1
  else
    run_cargo_with_timeout_override "${timeout_override}" test --workspace --verbose --lib --bins --features test-support \
      || return 1
  fi
}

run_integration_tests() {
  log_step "cargo +${DEFAULT_TOOLCHAIN} test --workspace --exclude fraktor-e2e-tests --verbose --tests --examples --features test-support"
  local timeout_override="${CI_CHECK_GUARD_TIMEOUT_INTEGRATION_SEC:-${CI_CHECK_GUARD_TIMEOUT_SEC}}"
  if [[ "${timeout_override}" == "0" ]]; then
    run_cargo test --workspace --exclude fraktor-e2e-tests --verbose --tests --examples --features test-support || return 1
  else
    run_cargo_with_timeout_override "${timeout_override}" test --workspace --exclude fraktor-e2e-tests --verbose --tests --examples --features test-support || return 1
  fi
  run_e2e_tests || return 1
}

run_e2e_tests() {
  log_step "cargo +${DEFAULT_TOOLCHAIN} test -p fraktor-e2e-tests --verbose --tests --features test-support"
  local timeout_override="${CI_CHECK_GUARD_TIMEOUT_INTEGRATION_SEC:-${CI_CHECK_GUARD_TIMEOUT_SEC}}"
  if [[ "${timeout_override}" == "0" ]]; then
    run_cargo test -p fraktor-e2e-tests --verbose --tests --features test-support || return 1
  else
    run_cargo_with_timeout_override "${timeout_override}" test -p fraktor-e2e-tests --verbose --tests --features test-support || return 1
  fi
}

run_tests() {
  run_unit_tests || return 1
  run_integration_tests || return 1
}

check_unit_sleep() {
  log_step "unit テスト内の実時間 sleep/timeout 使用を検査"
  if ! command -v rg >/dev/null 2>&1; then
    echo "error: rg (ripgrep) が必要ですが見つかりませんでした。" >&2
    return 1
  fi
  local -a scan_dirs=(
    modules/actor-core/src/
    modules/actor-adaptor-std/src/
    modules/stream-core/src/
    modules/stream-adaptor-std/src/
    modules/remote/src/
    modules/cluster-core/src/
    modules/cluster-adaptor-std/src/
  )
  local -a rg_globs=(
    --glob '**/tests.rs'
    --glob '**/tests/*.rs'
  )
  local -a rg_excludes=(
    --glob '!modules/remote/src/std/transport/**'
    --glob '!modules/remote/tests/**'
    --glob '!modules/cluster-adaptor-std/src/std/tokio_gossip_transport/**'
    --glob '!modules/actor-core/src/core/kernel/system/coordinated_shutdown/tests.rs'
    --glob '!modules/actor-core/src/core/kernel/dispatch/dispatcher/tests.rs'
    --glob '!modules/actor-core/src/core/typed/dsl/routing/scatter_gather_first_completed_router_builder/tests.rs'
    --glob '!modules/actor-core/src/core/typed/dsl/routing/tail_chopping_router_builder/tests.rs'
    --glob '!modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tests/test_tick_driver.rs'
  )

  local violations=""

  # Phase 1: thread::sleep は常に禁止（tokio 仮想時間の影響を受けない）
  local thread_violations=""
  thread_violations=$(rg -n 'thread::sleep' \
    "${rg_globs[@]}" "${rg_excludes[@]}" \
    "${scan_dirs[@]}" 2>/dev/null || true)
  if [[ -n "${thread_violations}" ]]; then
    violations+="${thread_violations}"$'\n'
  fi

  # Phase 2: tokio::time::{sleep,timeout} は start_paused のないファイルでのみ禁止
  local tokio_time_files=""
  tokio_time_files=$(rg -l 'tokio::time::sleep|tokio::time::timeout' \
    "${rg_globs[@]}" "${rg_excludes[@]}" \
    "${scan_dirs[@]}" 2>/dev/null || true)
  for file in ${tokio_time_files}; do
    if ! rg -q 'start_paused' "${file}" 2>/dev/null; then
      local file_violations=""
      file_violations=$(rg -n 'tokio::time::sleep|tokio::time::timeout' "${file}" 2>/dev/null || true)
      if [[ -n "${file_violations}" ]]; then
        violations+="${file_violations}"$'\n'
      fi
    fi
  done

  violations=$(echo -n "${violations}" | sed '/^$/d')
  if [[ -n "${violations}" ]]; then
    echo "error: unit テストパスに実時間 sleep/timeout が検出されました:" >&2
    echo "${violations}" >&2
    echo "allowlist に追加するか、fake clock / manual tick / start_paused に置き換えてください。" >&2
    return 1
  fi
}

run_actor_path_e2e() {
  log_step "cargo +${DEFAULT_TOOLCHAIN} test -p fraktor-actor-core-rs --test actor_path_e2e -- --nocapture"
  run_cargo test -p fraktor-actor-core-rs --test actor_path_e2e -- --nocapture || return 1
}

run_examples() {
  local python_bin=""
  python_bin="$(resolve_python3_bin)" || return 1

  local rustflags_value
  if [[ -n "${RUSTFLAGS-}" ]]; then
    rustflags_value="${RUSTFLAGS} -Dwarnings --force-warn deprecated"
  else
    rustflags_value="-Dwarnings --force-warn deprecated"
  fi

  local example_file=""
  example_file="$(mktemp)" || return 1
  local metadata_file=""
  metadata_file="$(mktemp)" || {
    rm -f "${example_file}"
    return 1
  }
  if ! env -u CARGO_TARGET_DIR CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" "${DEFAULT_CARGO_CMD[@]}" metadata --format-version 1 --no-deps > "${metadata_file}"; then
    [[ -n "${example_file}" ]] && rm -f "${example_file}"
    [[ -n "${metadata_file}" ]] && rm -f "${metadata_file}"
    echo "エラー: cargo metadata の取得に失敗しました。" >&2
    return 1
  fi
  if ! "${python_bin}" - "${metadata_file}" <<'PY' >"${example_file}"; then
import json
import sys

try:
    with open(sys.argv[1], encoding="utf-8") as f:
        metadata = json.load(f)
except Exception as exc:
    print(f"metadata error: {exc}", file=sys.stderr)
    sys.exit(1)

workspace = set(metadata.get("workspace_members", []))
for package in metadata.get("packages", []):
    if package.get("id") not in workspace:
        continue
    name = package.get("name")
    if not name:
        continue
    for target in package.get("targets", []):
        kinds = target.get("kind", [])
        if "example" not in kinds:
            continue
        target_name = target.get("name")
        if target_name:
            # 必要なfeatureを判定
            required_features = target.get("required-features", [])
            features_str = ",".join(required_features) if required_features else ""
            print(f"{name}\t{target_name}\t{features_str}")
PY
    [[ -n "${example_file}" ]] && rm -f "${example_file}"
    [[ -n "${metadata_file}" ]] && rm -f "${metadata_file}"
    return 1
  fi
  [[ -n "${metadata_file}" ]] && rm -f "${metadata_file}"

  local had_examples=""
  while IFS=$'\t' read -r package_name example_name features; do
    if [[ -z "${package_name}" || -z "${example_name}" ]]; then
      continue
    fi
    had_examples="yes"

    local -a cargo_args=(run --package "${package_name}" --example "${example_name}")
    if [[ -n "${features}" ]]; then
      cargo_args+=(--features "${features}")
      log_step "cargo +${DEFAULT_TOOLCHAIN} -v run --package ${package_name} --example ${example_name} --features ${features}"
    else
      log_step "cargo +${DEFAULT_TOOLCHAIN} -v run --package ${package_name} --example ${example_name}"
    fi
    RUSTFLAGS="${rustflags_value}" run_cargo "${cargo_args[@]}" \
      || {
        [[ -n "${example_file}" ]] && rm -f "${example_file}"
        return 1
      }
    echo
  done <"${example_file}"

  [[ -n "${example_file}" ]] && rm -f "${example_file}"

  if [[ -z "${had_examples}" ]]; then
    echo "info: 実行可能な example が見つかりませんでした" >&2
  fi
}

run_perf() {
  log_step "cargo test -p fraktor-actor-core-rs stress_scheduler_handles_"
  run_cargo test -p fraktor-actor-core-rs stress_scheduler_handles_ || return 1

  log_step "cargo +${DEFAULT_TOOLCHAIN} bench -p fraktor-actor-adaptor-std-rs --bench actor_baseline --features test-support,tokio-executor -- --warm-up-time 0.1 --measurement-time 0.2 --sample-size 10"
  run_cargo bench -p fraktor-actor-adaptor-std-rs --bench actor_baseline --features test-support,tokio-executor -- --warm-up-time 0.1 --measurement-time 0.2 --sample-size 10 || return 1
}

run_all() {
  # Phase 1: ゲート（直列・高速）
  log_step "=== Phase 1: ゲート (fmt, check_unit_sleep) ==="
  run_fmt || return 1
  check_unit_sleep || return 1

  # Phase 2: lint 群（並列）
  log_step "=== Phase 2: lint 並列 (dylint | clippy | no-std | doc) ==="
  PARALLEL_PIDS=()
  PARALLEL_LABELS=()
  start_parallel_phase "dylint" "dylint" run_dylint
  start_parallel_phase "clippy" "clippy" run_clippy
  start_parallel_phase "no-std" "no-std" run_no_std
  start_parallel_phase "doc" "doc" run_doc_tests
  wait_parallel_cargo || return 1

  # Phase 3: テスト群（並列）
  log_step "=== Phase 3: テスト並列 (unit-test | integration-test) ==="
  PARALLEL_PIDS=()
  PARALLEL_LABELS=()
  start_parallel_phase "unit-test" "unit-test" run_unit_tests
  start_parallel_phase "integration-test" "integration-test" run_integration_tests
  wait_parallel_cargo || return 1

  # Phase 4: examples（直列・最後）
  log_step "=== Phase 4: examples ==="
  run_examples || return 1
}

main() {
  mkdir -p "${REPO_ROOT}/target"
  local lockfile="${REPO_ROOT}/target/.ci-check.lock"
  while true; do
    if ( set -o noclobber; printf '%s\n' "$$" > "${lockfile}" ) 2>/dev/null; then
      break
    fi

    local lock_pid=""
    lock_pid="$(cat "${lockfile}" 2>/dev/null || true)"
    if [[ -n "${lock_pid}" ]] && kill -0 "${lock_pid}" 2>/dev/null; then
      echo "error: ci-check.sh は既に実行中です (PID: ${lock_pid})。AIエージェントは二重起動せず、先行プロセスの完了を待ってから同じ作業を再実行してください。" >&2
      return 1
    fi

    rm -f "${lockfile}" >/dev/null 2>&1 || true
  done
  trap "rm -f '${lockfile}'" EXIT

  if [[ -n "${CARGO_BUILD_JOBS:-}" ]]; then
    echo "info: CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}" >&2
  fi

  if [[ -x "${SCRIPT_DIR}/check_modrs.sh" ]]; then
    "${SCRIPT_DIR}/check_modrs.sh"
  fi

  clean_stale_lint_targets

  if [[ $# -gt 0 && "$1" == "ai" ]]; then
    shift
    enable_ai_mode
    if [[ $# -eq 0 ]]; then
      set -- all
    fi
  fi

  if [[ $# -eq 0 ]]; then
    run_with_heartbeat "ci-check all" run_all || return 1
    return
  fi

  while [[ $# -gt 0 ]]; do
    case "$1" in
      lint)
        run_lint || return 1
        shift
        ;;
      fmt)
        run_fmt || return 1
        shift
        ;;
      dylint)
        shift
        local -a lint_args=()
        while [[ $# -gt 0 ]]; do
          case "$1" in
            lint|fmt|dylint|dylint:*|clippy|no-std|nostd|std|embedded|embassy|test|tests|workspace|unit-test|unit-tests|integration-test|integration-tests|check-unit-sleep|all)
              break
              ;;
            --)
              shift
              break
              ;;
            *)
              lint_args+=("$1")
              shift
              ;;
          esac
        done
        if [[ ${#lint_args[@]} -eq 0 ]]; then
          if ! run_dylint; then
            return 1
          fi
        else
          if ! run_dylint "${lint_args[@]}"; then
            return 1
          fi
        fi
        ;;
      dylint:*)
        local spec="${1#dylint:}"
        local -a lint_args=()
        IFS=',' read -r -a lint_args <<< "${spec}"
        if [[ ${#lint_args[@]} -eq 0 || ( ${#lint_args[@]} -eq 1 && -z "${lint_args[0]}" ) ]]; then
          if ! run_dylint; then
            return 1
          fi
        else
          if ! run_dylint "${lint_args[@]}"; then
            return 1
          fi
        fi
        shift
        ;;
      clippy)
        run_clippy || return 1
        shift
        ;;
      no-std|nostd)
        run_no_std || return 1
        shift
        ;;
      std)
        run_std || return 1
        shift
        ;;
      doc|docs)
        run_doc_tests || return 1
        shift
        ;;
      examples|example)
        run_examples || return 1
        shift
        ;;
      embedded|embassy)
        run_embedded || return 1
        shift
        ;;
      unit-test|unit-tests)
        run_unit_tests || return 1
        shift
        ;;
      integration-test|integration-tests)
        run_integration_tests || return 1
        shift
        ;;
      e2e-test|e2e-tests)
        run_e2e_tests || return 1
        shift
        ;;
      check-unit-sleep)
        check_unit_sleep || return 1
        shift
        ;;
      test|tests|workspace)
        run_tests || return 1
        shift
        ;;
      perf|bench|performance)
        run_perf || return 1
        shift
        ;;
      actor-path-e2e)
        run_actor_path_e2e || return 1
        shift
        ;;
      all)
        run_with_heartbeat "ci-check all" run_all || return 1
        shift
        ;;
      --help|-h|help)
        usage
        return 0
        ;;
      --)
        shift
        if ! run_with_heartbeat "ci-check all" run_all; then
          return 1
        fi
        ;;
      *)
        usage
        return 1
        ;;
    esac
  done
  return 0
}

main "$@"
