## Context

`scripts/ci-check.sh` の `run_all()` 関数は8つのチェックを直列に実行している。一方、GitHub Actions CI は `matrix` ストラテジーで lint（clippy/dylint）とテスト（no_std/std/workspace）を並列ランナーで同時実行するため高速。

スクリプト内には既に `start_parallel_cargo()` / `wait_parallel_cargo()` という並列実行インフラが存在し、`run_no_std()` と `run_std()` 内で活用されている。これを `run_all()` レベルに拡張する。

## Goals / Non-Goals

**Goals:**
- `run_all()` の実行時間を短縮する（目標: 現状の60-70%）
- 既存の `start_parallel_cargo` / `wait_parallel_cargo` インフラを再利用する
- 個別コマンド実行（`ci-check.sh clippy` 等）の動作を変更しない

**Non-Goals:**
- GitHub Actions CI ワークフロー（`.github/workflows/ci.yml`）の変更
- `CARGO_BUILD_JOBS` のチューニング
- sccache や incremental compilation の導入
- 並列度の動的調整（CPUコア数に応じた制御等）

## Decisions

### 決定1: 3フェーズ構成

`run_all()` を3フェーズに分割する。

```
Phase 1 (ゲート・直列):  fmt, check_unit_sleep
Phase 2 (並列):          dylint | clippy | no-std | doc
Phase 3 (並列):          unit-test | integration-test
```

**理由**: fmt はフォーマット修正を行うため最初に実行すべき（他ステップの差分に影響）。lint群は互いに独立。テスト群はlint通過後に実行することで、lint失敗時の無駄なビルドを避ける。

**代替案**: 全ステップを一気に並列化する案も検討したが、lint失敗時にテストが無駄に走るコストが大きいため却下。

### 決定2: `start_parallel_cargo` の拡張方法

現状の `start_parallel_cargo` はサブシェルで `run_cargo` を呼び出し、`CARGO_TARGET_DIR` を分離している。各ラッパー関数（`run_clippy` 等）は直接 `run_cargo` を呼んでいるため、これらをラップする形で並列化する。

具体的には、`run_all` 内で `start_parallel_cargo` を直接呼ぶのではなく、各 `run_*` 関数ごとにサブシェル + 別ターゲットディレクトリで実行する `start_parallel_phase` ヘルパーを新設する。

```bash
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
```

**理由**: 既存の `start_parallel_cargo` は単一の `run_cargo` 呼び出し用。`run_dylint` のように内部で複数の cargo コマンドを呼ぶ関数には使えないため、関数単位で並列化するヘルパーが必要。

### 決定3: dylint のターゲットディレクトリ

dylint は内部で `CARGO_TARGET_DIR=target-dylint` を使用している（`run_cargo` とは別ディレクトリ）。並列実行時は `ci-check/dylint` シャードの中で通常通り `target-dylint` が使われるため、clippy 等との競合は発生しない。

## Risks / Trade-offs

- **ディスク使用量の増加** → 並列ジョブごとに別ターゲットディレクトリを使用するため、ビルド成果物が重複する。`target/ci-check/` 以下に一時ディレクトリが増える。完了後のクリーンアップは初期実装では行わない（手動 `cargo clean` で対応可能）。
- **CPU/メモリ圧迫** → Phase 2 で4プロセスが同時にビルドを行う。`CARGO_BUILD_JOBS=4` の場合、最大16スレッドが動く可能性がある。ローカルマシンのコア数が少ない場合はスラッシングのリスクがある。→ 初期実装では並列度を固定し、問題が出た場合に `CI_CHECK_PARALLEL_JOBS` 環境変数で制御可能にすることを検討する。
- **エラー出力の混在** → 並列ジョブの stderr/stdout が混ざる可能性がある。→ `wait_parallel_cargo` が既にジョブごとの成否を報告する仕組みを持っているため、致命的ではない。
