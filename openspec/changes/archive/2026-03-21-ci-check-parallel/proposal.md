## Why

`scripts/ci-check.sh ai all` はすべてのチェック（fmt, dylint, clippy, no-std, doc, unit-test, integration-test）を直列実行しているため、GitHub Actions CI より大幅に遅い。GitHub Actions はジョブを並列ランナーで同時実行することで高速化しているが、ローカルの `run_all` にはその並列化が適用されていない。

## What Changes

- `run_all()` 関数を3フェーズ構成に再構成し、独立したステップを並列実行する
  - Phase 1（ゲート）: `fmt` + `check_unit_sleep`（直列、高速）
  - Phase 2（並列）: `dylint` | `clippy` | `no-std` | `doc`（別 `CARGO_TARGET_DIR` で同時実行）
  - Phase 3（並列）: `unit-test` | `integration-test`（別 `CARGO_TARGET_DIR` で同時実行）
- 既存の `start_parallel_cargo` / `wait_parallel_cargo` インフラを活用する
- `run_all` 以外の個別コマンド実行（`ci-check.sh clippy` 等）の動作は変更しない

## Capabilities

### New Capabilities
- `ci-check-parallel-phases`: `run_all` のフェーズ並列化。既存の並列cargoインフラ（`start_parallel_cargo` / `wait_parallel_cargo`）を拡張し、lint群・テスト群をフェーズごとに並列実行する

### Modified Capabilities

（なし）

## Impact

- 影響ファイル: `scripts/ci-check.sh`（`run_all` 関数の変更）
- ディスク使用量: 並列ジョブごとに別 `CARGO_TARGET_DIR` を使用するため、一時的にディスク使用量が増加する
- CPU/メモリ: ローカルマシンのリソースに依存。並列度が高すぎるとスラッシングの可能性あり
- 既存の個別コマンド実行には影響なし
