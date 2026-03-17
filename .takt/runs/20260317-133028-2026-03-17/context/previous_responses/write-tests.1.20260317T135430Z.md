変更内容は正しく適用されています。

## 作業結果

計画に基づき、7つのテストファイルを変更しました。プロダクションコードは変更していません。テストは implement ムーブメントでのプロダクションコード変更（`CircuitBreaker::new_with_clock` 追加、Cargo.toml への `test-util` feature 追加）後にパスすることを前提としています。

### 変更の概要

**A. 単体テスト層から壁時計待ちを除去:**

| ファイル | 変更内容 |
|---------|---------|
| `circuit_breaker/tests.rs` | `thread::sleep` → `FakeClock` パターン。`CircuitBreaker::new_with_clock` を使い、`clock.advance()` で決定的に時間を制御。新規テスト `open_remains_open_before_reset_timeout` と `open_error_reports_correct_remaining_duration` を追加 |
| `circuit_breaker_shared/tests.rs` | `tokio::time::sleep` → `FakeClock` パターン。`CircuitBreakerShared::new_with_clock` を使用。cancel safety テスト内の 60s sleep はキャンセル対象のため維持 |
| `scheduler/tick/tests.rs` | `multi_thread` → `current_thread, start_paused = true`。`tokio::task::yield_now()` を追加してバックグラウンドタスクの実行を保証 |
| `system/base/tests.rs` | 不要な `tokio::time::sleep(20ms)` を削除。assert 対象（system name）は構築時に確定するため時間経過と無関係 |

**B. 統合テスト層の待ち方を改善:**

| ファイル | 変更内容 |
|---------|---------|
| `endpoint_transport_bridge/tests.rs` | sleep 使用テスト8件に `start_paused = true` を追加。periodic テストに `yield_now()` を追加。`inbound_handler_rejects_frames_when_queue_is_full` は `thread::sleep` 依存のため実時間統合テストとして維持 |
| `quickstart.rs` | `start_paused = true` を追加 |
| `multi_node_scenario_integration.rs` | `start_paused = true` を追加 |

## 変更内容
- `modules/actor/src/std/pattern/circuit_breaker/tests.rs` — FakeClock 導入、sleep 除去、境界値テスト追加
- `modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs` — FakeClock 導入、sleep 除去
- `modules/actor/src/std/scheduler/tick/tests.rs` — paused Tokio clock 移行
- `modules/actor/src/std/system/base/tests.rs` — 不要 sleep 削除
- `modules/remote/src/std/endpoint_transport_bridge/tests.rs` — start_paused 適用（8テスト）
- `modules/remote/tests/quickstart.rs` — start_paused 適用
- `modules/remote/tests/multi_node_scenario_integration.rs` — start_paused 適用