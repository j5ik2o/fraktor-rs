## 1. start_parallel_phase ヘルパーの追加

- [x] 1.1 `start_parallel_phase` 関数を `scripts/ci-check.sh` に追加する（`start_parallel_cargo` の直後に配置）。引数: label, shard, func。サブシェル内で `CARGO_TARGET_DIR` を設定し、指定関数をバックグラウンド実行する
- [x] 1.2 `start_parallel_phase` が既存の `PARALLEL_PIDS` / `PARALLEL_LABELS` 配列を正しく更新することを確認する

## 2. run_all の3フェーズ化

- [x] 2.1 `run_all()` を Phase 1（直列: `run_fmt`, `check_unit_sleep`）に変更する
- [x] 2.2 Phase 2（並列: `run_dylint`, `run_clippy`, `run_no_std`, `run_doc_tests`）を `start_parallel_phase` + `wait_parallel_cargo` で実装する
- [x] 2.3 Phase 3（並列: `run_unit_tests`, `run_integration_tests`）を `start_parallel_phase` + `wait_parallel_cargo` で実装する
- [x] 2.4 各フェーズ失敗時に後続フェーズを実行しないガードを入れる

## 3. 動作確認

- [x] 3.1 `ci-check.sh ai all` を実行し、全フェーズが正常に完了することを確認する
- [x] 3.2 `ci-check.sh clippy` 等の個別コマンドが従来通り動作することを確認する
- [x] 3.3 Phase 2 で意図的にエラーを起こし、Phase 3 が実行されないことを確認する
