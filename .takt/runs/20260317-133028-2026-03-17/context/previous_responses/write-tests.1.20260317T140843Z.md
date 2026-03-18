test-scope.md と test-decisions.md は既にplanムーブメントで適切に作成されており、テストファイルの変更も全てスコープ通りに適用済みです。追加の変更は不要です。

## 作業結果

全テストファイルの変更は前回イテレーションで既に適用済みであることを確認しました。test-scope.md に記載された7ファイル全てが計画通りに変更されています。

### 確認した変更内容

| ファイル | 変更内容 | 状態 |
|----------|----------|------|
| `modules/actor/src/std/pattern/circuit_breaker/tests.rs` | FakeClock導入、`thread::sleep` → `clock.advance()`、境界値テスト2件追加（`open_remains_open_before_reset_timeout`, `open_error_reports_correct_remaining_duration`） | ✅ 適用済み |
| `modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs` | FakeClock導入、`tokio::time::sleep` → `clock.advance()`（`call_recovers_after_reset_timeout`, `half_open_failure_reopens`, `open_error_contains_remaining_duration`, `cancel_during_half_open_records_failure`） | ✅ 適用済み |
| `modules/actor/src/std/scheduler/tick/tests.rs` | `multi_thread` → `current_thread, start_paused = true`、`yield_now()` 追加 | ✅ 適用済み |
| `modules/actor/src/std/system/base/tests.rs` | 不要な `tokio::time::sleep(20ms)` と `Duration` import 削除 | ✅ 適用済み |
| `modules/remote/src/std/endpoint_transport_bridge/tests.rs` | 8テストに `start_paused = true` 追加、periodic テスト2件に `yield_now()` 追加 | ✅ 適用済み |
| `modules/remote/tests/quickstart.rs` | `start_paused = true` 追加 | ✅ 適用済み |
| `modules/remote/tests/multi_node_scenario_integration.rs` | `start_paused = true` 追加 | ✅ 適用済み |

## 変更内容

追加の変更なし。全テストファイルは前回イテレーションの成果物として既に正しく変更されています。次の `implement` ムーブメントで以下のプロダクションコード変更が必要です：

1. `CircuitBreaker::new_with_clock` / `CircuitBreakerShared::new_with_clock` の実装追加
2. `modules/actor/Cargo.toml` と `modules/remote/Cargo.toml` に tokio `test-util` feature 追加
3. `scripts/ci-check.sh` の unit/integration テスト分離