# 実装スコープ

## 対象
- `modules/actor/src/std/pattern/circuit_breaker.rs` — `new_with_clock` コンストラクタ追加
- `modules/actor/src/std/pattern/circuit_breaker_shared.rs` — `new_with_clock` コンストラクタ追加
- `modules/actor/Cargo.toml` — tokio `test-util` feature 追加
- `modules/remote/Cargo.toml` — tokio `test-util` feature 追加
- `scripts/ci-check.sh` — unit/integration テスト分離

## 非対象
- テストファイル（前回の write_tests ムーブメントで作成済み）
- `modules/actor/src/std/pattern/circuit_breaker/tests.rs`
- `modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs`
- `modules/actor/src/std/scheduler/tick/tests.rs`
- `modules/actor/src/std/system/base/tests.rs`
- `modules/remote/src/std/endpoint_transport_bridge/tests.rs`
- `modules/remote/tests/quickstart.rs`
- `modules/remote/tests/multi_node_scenario_integration.rs`

## スコープ判断の理由
- テストは write_tests ムーブメントで既に作成済みであり、本ムーブメントではテストがパスするためのプロダクションコード変更と設定変更のみが対象
- `new_with_clock` はテストで FakeClock を注入するために必要な新コンストラクタ
- Cargo.toml の `test-util` feature は `tokio::time::pause`/`start_paused` をテストで使うために必要