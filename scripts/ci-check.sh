#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

THUMB_TARGETS=("thumbv6m-none-eabi" "thumbv8m.main-none-eabi")
declare -a HARDWARE_PACKAGES=()
DEFAULT_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-nightly}"
FMT_TOOLCHAIN="${FMT_TOOLCHAIN:-nightly}"

usage() {
  cat <<'EOF'
使い方: scripts/ci-check.sh [コマンド...]
  lint                   : cargo +nightly fmt -- --check を実行します
  dylint [lint...]       : カスタムリントを実行します (デフォルトはすべて、例: dylint mod-file-lint)
                           CSV 形式のショートハンドも利用可能です (例: dylint:mod-file-lint,module-wiring-lint)
  clippy                 : cargo clippy --workspace --all-targets -- -D warnings を実行します
  no-std                 : no_std 対応チェック (core/utils) を実行します
  std                    : std フィーチャーでのテストを実行します
  doc                    : ドキュメントテストを test-support フィーチャー付きで実行します
  examples               : ワークスペース配下の examples をビルドします
  embedded / embassy     : embedded 系 (utils / actor) のチェックとテストを実行します
  test                   : ワークスペース全体のテストを実行します
  perf                   : Scheduler ストレス／ベンチマークテストを実行します
  actor-path-e2e         : fraktor-actor-rs の actor_path_e2e テストを単体実行します
  all                    : 上記すべてを順番に実行します (引数なし時と同じ)
複数指定で部分実行が可能です (例: scripts/ci-check.sh lint dylint module-wiring-lint)
EOF
}

log_step() {
  printf '==> %s\n' "$1"
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
  if [[ -n "${DEFAULT_TOOLCHAIN}" ]]; then
    cmd=(cargo "+${DEFAULT_TOOLCHAIN}" "$@")
  else
    cmd=(cargo "$@")
  fi

  if ! "${cmd[@]}"; then
    echo "error: ${cmd[*]}" >&2
    return 1
  fi
}

ensure_target_installed() {
  local target="$1"

  if rustup target list --installed --toolchain "${DEFAULT_TOOLCHAIN}" | grep -qx "${target}"; then
    return 0
  fi

  if [[ -n "${CI:-}" ]]; then
    echo "info: installing target ${target} for toolchain ${DEFAULT_TOOLCHAIN}" >&2
    if rustup target add --toolchain "${DEFAULT_TOOLCHAIN}" "${target}"; then
      return 0
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

  if ! component_list=$(rustup component list --toolchain "${DEFAULT_TOOLCHAIN}" 2>/dev/null); then
    echo "エラー: rustup component list の取得に失敗しました" >&2
    return 1
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
    if rustup component add --toolchain "${DEFAULT_TOOLCHAIN}" "${component}"; then
      echo "info: ${component} のインストールが完了しました。" >&2
    else
      echo "エラー: ${component} のインストールに失敗しました。" >&2
      return 1
    fi
  done

  return 0
}

ensure_dylint_installed() {
  if command -v cargo-dylint >/dev/null 2>&1; then
    return 0
  fi

  echo "info: cargo-dylint がインストールされていないため、インストールします..." >&2
  if cargo install cargo-dylint; then
    echo "info: cargo-dylint のインストールが完了しました。" >&2
    return 0
  fi

  echo "エラー: cargo-dylint のインストールに失敗しました。" >&2
  echo "手動でインストールする場合: cargo install cargo-dylint" >&2
  return 1
}

run_lint() {
  log_step "cargo +${FMT_TOOLCHAIN} fmt -- --check"
  cargo "+${FMT_TOOLCHAIN}" fmt --all -- --check || return 1
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
        echo "利用例: scripts/ci-check.sh dylint -n mod-file-lint -m fraktor-actor-rs" >&2
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
    "module-wiring-lint:lints/module-wiring-lint"
    "type-per-file-lint:lints/type-per-file-lint"
    "tests-location-lint:lints/tests-location-lint"
    "use-placement-lint:lints/use-placement-lint"
    "rustdoc-lint:lints/rustdoc-lint"
    "cfg-std-forbid-lint:lints/cfg-std-forbid-lint"
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
  toolchain="nightly-$(rustc +nightly -vV | awk '/^host:/{print $2}')"
  local -a lib_dirs=()
  local -a dylint_args=()

  local entry
  for entry in "${selected[@]}"; do
    local crate="${entry%%:*}"
    local lint_path="${entry#*:}"
    local lib_name="${crate//-/_}"

    log_step "cargo +nightly build --manifest-path ${lint_path}/Cargo.toml --release"
    CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" cargo +nightly build --manifest-path "${lint_path}/Cargo.toml" --release || return 1

    log_step "cargo +nightly test --manifest-path ${lint_path}/Cargo.toml -- test ui -- --quiet"
    CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" cargo +nightly test --manifest-path "${lint_path}/Cargo.toml" -- test ui -- --quiet || return 1

    local dylib_ext
    dylib_ext="$(get_dylib_extension)"
    local target_dir="${lint_path}/target/release"
    local plain_lib="${target_dir}/lib${lib_name}.${dylib_ext}"
    local tagged_lib="${target_dir}/lib${lib_name}@${toolchain}.${dylib_ext}"

    if [[ -f "${plain_lib}" ]]; then
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
    rustflags_value="${RUSTFLAGS} -Dwarnings -Adeprecated"
  else
    rustflags_value="-Dwarnings -Adeprecated"
  fi

  local -a common_dylint_args=("${dylint_args[@]}" "--no-metadata")
  local -a hardware_packages=()
  if [[ ${HARDWARE_PACKAGES+set} == set && ${#HARDWARE_PACKAGES[@]} -gt 0 ]]; then
    hardware_packages=("${HARDWARE_PACKAGES[@]}")
  fi
  local -a main_package_args=()
  local -a hardware_targets=()
  local -a feature_packages=("fraktor-actor-rs=tokio-executor")

  if [[ ${#package_args[@]} -eq 0 ]]; then
    if ! command -v python3 >/dev/null 2>&1; then
      echo "エラー: python3 が必要ですが見つかりませんでした。" >&2
      return 1
    fi
    local -a python_cmd=(python3 -)
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
import subprocess
import sys

metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--no-deps"], text=True))
hardware = set(sys.argv[1:])
for package in metadata.get("packages", []):
    name = package.get("name")
    if not name or name in hardware:
        continue
    print(name)
PY
    )
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
    log_step "cargo +${DEFAULT_TOOLCHAIN} dylint ${log_main}${log_trailing} (RUSTFLAGS=${rustflags_value})"
    if [[ ${#trailing_args[@]} -gt 0 ]]; then
      RUSTFLAGS="${rustflags_value}" DYLINT_LIBRARY_PATH="${dylint_library_path}" CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" run_cargo dylint "${main_invocation[@]}" -- "${trailing_args[@]}" || return 1
    else
      RUSTFLAGS="${rustflags_value}" DYLINT_LIBRARY_PATH="${dylint_library_path}" CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" run_cargo dylint "${main_invocation[@]}" || return 1
    fi
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
      log_step "cargo +${DEFAULT_TOOLCHAIN} dylint ${log_pkg}${log_trailing} (RUSTFLAGS=${rustflags_value})"
      if [[ ${#trailing_args[@]} -gt 0 ]]; then
        RUSTFLAGS="${rustflags_value}" DYLINT_LIBRARY_PATH="${dylint_library_path}" CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" run_cargo dylint "${pkg_invocation[@]}" -- "${trailing_args[@]}" || return 1
      else
        RUSTFLAGS="${rustflags_value}" DYLINT_LIBRARY_PATH="${dylint_library_path}" CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" run_cargo dylint "${pkg_invocation[@]}" || return 1
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
      log_step "cargo +${DEFAULT_TOOLCHAIN} dylint ${log_feature} (RUSTFLAGS=${rustflags_value})"
      RUSTFLAGS="${rustflags_value}" DYLINT_LIBRARY_PATH="${dylint_library_path}" CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" run_cargo dylint "${feature_invocation[@]}" -- "${feature_trailing[@]}" || return 1
    done
  fi
}

run_clippy() {
  log_step "cargo +${DEFAULT_TOOLCHAIN} clippy --workspace --all-targets -- -D warnings"
  run_cargo clippy --workspace --all-targets -- -D warnings || return 1
}

run_no_std() {
  log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-utils-rs --no-default-features --features alloc"
  run_cargo check -p fraktor-utils-rs --no-default-features --features alloc || return 1

  log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-rs --no-default-features"
  run_cargo check -p fraktor-actor-rs --no-default-features || return 1

  log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-rs --no-default-features"
  run_cargo check -p fraktor-rs --no-default-features || return 1

  local thumb_target="thumbv8m.main-none-eabi"
  if ensure_target_installed "${thumb_target}"; then
    log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-utils-rs --no-default-features --features alloc --target ${thumb_target}"
    run_cargo check -p fraktor-utils-rs --no-default-features --features alloc --target "${thumb_target}" || return 1

    log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-rs --no-default-features --target ${thumb_target}"
    run_cargo check -p fraktor-actor-rs --no-default-features --target "${thumb_target}" || return 1
  fi
}

run_std() {
  log_step "cargo +${DEFAULT_TOOLCHAIN} test -p fraktor-utils-rs"
  run_cargo test -p fraktor-utils-rs || return 1

  log_step "cargo +${DEFAULT_TOOLCHAIN} test -p fraktor-actor-rs --lib"
  run_cargo test -p fraktor-actor-rs --lib || return 1

  log_step "cargo +${DEFAULT_TOOLCHAIN} test -p fraktor-rs --lib"
  run_cargo test -p fraktor-rs --lib || return 1
}

run_doc_tests() {
  log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-rs --no-default-features"
  run_cargo check -p fraktor-actor-rs --no-default-features || return 1
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
#    log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-rs --target ${target} --no-default-features --features alloc"
#    run_cargo check -p fraktor-actor-rs --target "${target}" --no-default-features --features alloc || return 1
#
#    log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-rs --target ${target} --no-default-features --features alloc"
#    run_cargo check -p fraktor-actor-rs --target "${target}" --no-default-features --features alloc || return 1
#
#    log_step "cargo +${DEFAULT_TOOLCHAIN} check -p fraktor-actor-embedded-rs --target ${target} --no-default-features --features alloc,embedded_rc"
#    run_cargo check -p fraktor-actor-embedded-rs --target "${target}" --no-default-features --features alloc,embedded_rc || return 1
#  done
# }

run_tests() {
  log_step "cargo +${DEFAULT_TOOLCHAIN} test --workspace --verbose --lib --bins --tests --benches --examples"
  run_cargo test --workspace --verbose --lib --bins --tests --benches --examples || return 1
}

run_actor_path_e2e() {
  log_step "cargo +${DEFAULT_TOOLCHAIN} test -p fraktor-actor-rs --test actor_path_e2e -- --nocapture"
  run_cargo test -p fraktor-actor-rs --test actor_path_e2e -- --nocapture || return 1
}

run_examples() {
  if ! command -v python3 >/dev/null 2>&1; then
    echo "エラー: python3 が必要ですが見つかりませんでした。" >&2
    return 1
  fi

  local example_file
  example_file="$(mktemp)"
  if ! python3 <<'PY' >"${example_file}"; then
import json
import subprocess
import sys

try:
    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            text=True,
        )
    )
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
    rm -f "${example_file}"
    return 1
  fi

  local had_examples=""
  while IFS=$'\t' read -r package_name example_name features; do
    if [[ -z "${package_name}" || -z "${example_name}" ]]; then
      continue
    fi
    had_examples="yes"

    local -a cargo_args=(run --package "${package_name}" --example "${example_name}")
    if [[ -n "${features}" ]]; then
      cargo_args+=(--features "${features}")
      log_step "cargo +${DEFAULT_TOOLCHAIN} run --package ${package_name} --example ${example_name} --features ${features} --quiet"
    else
      log_step "cargo +${DEFAULT_TOOLCHAIN} run --package ${package_name} --example ${example_name} --quiet"
    fi
    cargo_args+=(--quiet)

    local log_file
    log_file="$(mktemp)"
    if [[ -n "${DEFAULT_TOOLCHAIN}" ]]; then
      cargo "+${DEFAULT_TOOLCHAIN}" "${cargo_args[@]}" \
        >"${log_file}" 2>&1 || {
          cat "${log_file}"
          rm -f "${log_file}" "${example_file}"
          return 1
        }
    else
      cargo "${cargo_args[@]}" \
        >"${log_file}" 2>&1 || {
          cat "${log_file}"
          rm -f "${log_file}" "${example_file}"
          return 1
        }
    fi
    tail -n 5 "${log_file}" || true
    echo
    rm -f "${log_file}"
  done <"${example_file}"

  rm -f "${example_file}"

  if [[ -z "${had_examples}" ]]; then
    echo "info: 実行可能な example が見つかりませんでした" >&2
  fi
}

run_perf() {
  log_step "cargo test -p fraktor-actor-rs stress_scheduler_handles_"
  run_cargo test -p fraktor-actor-rs stress_scheduler_handles_ || return 1
}

run_all() {
  run_lint || return 1
  run_dylint || return 1
  run_clippy || return 1
  run_no_std || return 1
  run_std || return 1
  run_doc_tests || return 1
#  run_embedded || return 1
  run_perf || return 1
  run_tests || return 1
  run_actor_path_e2e || return 1
  run_examples || return 1
}

main() {
  if [[ -x "${SCRIPT_DIR}/check_modrs.sh" ]]; then
    "${SCRIPT_DIR}/check_modrs.sh"
  fi

  clean_stale_lint_targets

  if [[ $# -eq 0 ]]; then
    run_all || return 1
    return
  fi

  while [[ $# -gt 0 ]]; do
    case "$1" in
      lint)
        run_lint || return 1
        shift
        ;;
      dylint)
        shift
        local -a lint_args=()
        while [[ $# -gt 0 ]]; do
          case "$1" in
            lint|dylint|dylint:*|clippy|no-std|nostd|std|embedded|embassy|test|tests|workspace|all)
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
        run_all || return 1
        shift
        ;;
      --help|-h|help)
        usage
        return 0
        ;;
      --)
        shift
        if ! run_all; then
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
