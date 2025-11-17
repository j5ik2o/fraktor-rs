#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

# カバレッジツールのデフォルト設定
COVERAGE_TOOL="${COVERAGE_TOOL:-llvm-cov}"
OUTPUT_DIR="${COVERAGE_DIR:-target/coverage}"
OUTPUT_FORMAT="${COVERAGE_FORMAT:-html}"

usage() {
  cat <<'EOF'
使い方: scripts/coverage.sh [オプション]

オプション:
  --tool <tool>       カバレッジツールを指定 (llvm-cov, grcov) [デフォルト: llvm-cov]
  --format <format>   出力形式を指定 (html, lcov, json) [デフォルト: html]
  --output <dir>      出力ディレクトリを指定 [デフォルト: target/coverage]
  --open              HTML出力後にブラウザで開く
  --help, -h          このヘルプを表示

環境変数:
  COVERAGE_TOOL       カバレッジツール (llvm-cov, grcov)
  COVERAGE_FORMAT     出力形式 (html, lcov, json)
  COVERAGE_DIR        出力ディレクトリ

例:
  # デフォルト設定でHTML出力
  scripts/coverage.sh

  # lcov形式で出力
  scripts/coverage.sh --format lcov

  # grcovを使用してHTML出力
  scripts/coverage.sh --tool grcov --format html

  # HTML出力後にブラウザで開く
  scripts/coverage.sh --open
EOF
}

log_step() {
  printf '==> %s\n' "$1"
}

ensure_tool_installed() {
  local tool="$1"

  case "${tool}" in
    llvm-cov)
      if cargo llvm-cov --version >/dev/null 2>&1; then
        return 0
      fi
      log_step "cargo-llvm-cov をインストールしています..."
      cargo install cargo-llvm-cov || {
        echo "エラー: cargo-llvm-cov のインストールに失敗しました" >&2
        return 1
      }
      ;;
    grcov)
      if command -v grcov >/dev/null 2>&1; then
        return 0
      fi
      log_step "grcov をインストールしています..."
      cargo install grcov || {
        echo "エラー: grcov のインストールに失敗しました" >&2
        return 1
      }
      ;;
    *)
      echo "エラー: 未知のカバレッジツール '${tool}'" >&2
      return 1
      ;;
  esac
}

run_llvm_cov() {
  local format="$1"
  local output_dir="$2"

  log_step "cargo llvm-cov を使用してカバレッジを計測中..."

  mkdir -p "${output_dir}"

  # Main packages to measure coverage for (run separately to avoid SIGSEGV)
  local -a packages=(
    "fraktor-utils-core-rs"
    "fraktor-actor-core-rs"
    "fraktor-actor-std-rs"
  )

  case "${format}" in
    html)
      log_step "HTML形式でカバレッジレポートを生成: ${output_dir}/html"
      # Clean previous coverage data
      cargo llvm-cov clean --workspace || return 1

      # Run coverage for each package separately and accumulate
      for pkg in "${packages[@]}"; do
        log_step "  パッケージ ${pkg} のカバレッジを計測中..."
        cargo llvm-cov --lib -p "${pkg}" --no-report || return 1
      done

      # Generate combined HTML report
      cargo llvm-cov report --html --output-dir "${output_dir}/html" || return 1
      echo "カバレッジレポート: ${output_dir}/html/index.html"
      ;;
    lcov)
      log_step "LCOV形式でカバレッジレポートを生成: ${output_dir}/lcov.info"
      # Clean previous coverage data
      cargo llvm-cov clean --workspace || return 1

      # Run coverage for each package separately and accumulate
      for pkg in "${packages[@]}"; do
        log_step "  パッケージ ${pkg} のカバレッジを計測中..."
        cargo llvm-cov --lib -p "${pkg}" --no-report || return 1
      done

      # Generate combined LCOV report
      cargo llvm-cov report --lcov --output-path "${output_dir}/lcov.info" || return 1
      echo "カバレッジレポート: ${output_dir}/lcov.info"
      ;;
    json)
      log_step "JSON形式でカバレッジレポートを生成: ${output_dir}/coverage.json"
      # Clean previous coverage data
      cargo llvm-cov clean --workspace || return 1

      # Run coverage for each package separately and accumulate
      for pkg in "${packages[@]}"; do
        log_step "  パッケージ ${pkg} のカバレッジを計測中..."
        cargo llvm-cov --lib -p "${pkg}" --no-report || return 1
      done

      # Generate combined JSON report
      cargo llvm-cov report --json --output-path "${output_dir}/coverage.json" || return 1
      echo "カバレッジレポート: ${output_dir}/coverage.json"
      ;;
    *)
      echo "エラー: 未知の出力形式 '${format}'" >&2
      return 1
      ;;
  esac
}

run_grcov() {
  local format="$1"
  local output_dir="$2"

  log_step "grcov を使用してカバレッジを計測中..."

  mkdir -p "${output_dir}"

  # プロファイルデータをクリーンアップ
  find . -name "*.profraw" -delete

  # RUSTFLAGS を設定してテストを実行
  log_step "テストを実行してプロファイルデータを収集中..."
  RUSTFLAGS="-C instrument-coverage" \
  LLVM_PROFILE_FILE="${REPO_ROOT}/target/coverage/default_%m_%p.profraw" \
  cargo test --workspace --all-features || return 1

  # grcov でカバレッジレポートを生成
  case "${format}" in
    html)
      log_step "HTML形式でカバレッジレポートを生成: ${output_dir}/html"
      grcov "${REPO_ROOT}/target/coverage" \
        --binary-path "${REPO_ROOT}/target/debug/" \
        -s "${REPO_ROOT}" \
        -t html \
        --branch \
        --ignore-not-existing \
        --ignore "target/*" \
        --ignore "*/tests/*" \
        -o "${output_dir}/html" || return 1
      echo "カバレッジレポート: ${output_dir}/html/index.html"
      ;;
    lcov)
      log_step "LCOV形式でカバレッジレポートを生成: ${output_dir}/lcov.info"
      grcov "${REPO_ROOT}/target/coverage" \
        --binary-path "${REPO_ROOT}/target/debug/" \
        -s "${REPO_ROOT}" \
        -t lcov \
        --branch \
        --ignore-not-existing \
        --ignore "target/*" \
        --ignore "*/tests/*" \
        -o "${output_dir}/lcov.info" || return 1
      echo "カバレッジレポート: ${output_dir}/lcov.info"
      ;;
    *)
      echo "エラー: grcov では '${format}' 形式はサポートされていません" >&2
      echo "サポートされている形式: html, lcov" >&2
      return 1
      ;;
  esac
}

main() {
  local open_browser=""

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --tool)
        if [[ $# -lt 2 ]]; then
          echo "エラー: --tool にはツール名を指定してください" >&2
          return 1
        fi
        COVERAGE_TOOL="$2"
        shift 2
        ;;
      --format)
        if [[ $# -lt 2 ]]; then
          echo "エラー: --format には形式を指定してください" >&2
          return 1
        fi
        OUTPUT_FORMAT="$2"
        shift 2
        ;;
      --output)
        if [[ $# -lt 2 ]]; then
          echo "エラー: --output にはディレクトリを指定してください" >&2
          return 1
        fi
        OUTPUT_DIR="$2"
        shift 2
        ;;
      --open)
        open_browser="yes"
        shift
        ;;
      --help|-h|help)
        usage
        return 0
        ;;
      *)
        echo "エラー: 未知のオプション '$1'" >&2
        usage
        return 1
        ;;
    esac
  done

  # ツールのインストール確認
  ensure_tool_installed "${COVERAGE_TOOL}" || return 1

  # カバレッジ計測実行
  case "${COVERAGE_TOOL}" in
    llvm-cov)
      run_llvm_cov "${OUTPUT_FORMAT}" "${OUTPUT_DIR}" || return 1
      ;;
    grcov)
      run_grcov "${OUTPUT_FORMAT}" "${OUTPUT_DIR}" || return 1
      ;;
    *)
      echo "エラー: 未知のカバレッジツール '${COVERAGE_TOOL}'" >&2
      return 1
      ;;
  esac

  # ブラウザで開く
  if [[ -n "${open_browser}" && "${OUTPUT_FORMAT}" == "html" ]]; then
    local html_file="${OUTPUT_DIR}/html/index.html"
    if [[ -f "${html_file}" ]]; then
      log_step "ブラウザでレポートを開いています..."
      if command -v open >/dev/null 2>&1; then
        open "${html_file}"
      elif command -v xdg-open >/dev/null 2>&1; then
        xdg-open "${html_file}"
      else
        echo "警告: ブラウザを自動的に開けませんでした" >&2
        echo "手動で開いてください: ${html_file}" >&2
      fi
    fi
  fi

  log_step "カバレッジ計測が完了しました"
}

main "$@"
