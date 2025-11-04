# actor-core 受入テスト設計

## 目的
- `specs/001-add-actor-runtime` に記載された US1〜US3 の受入シナリオを `modules/actor-core/tests` で自動化し、ActorSystem（`NoStdToolbox`）のみで回帰できるようにする。
- protoactor-go / Apache Pekko を参考に導入した仕様（Mailbox FIFO、Supervisor 戦略、EventStream/Deadletter）の Rust 実装が逸脱していないかを CI で検証する。

-## スコープ
- 対象クレート: `cellactor-actor-core-rs`
- 対象機能: ActorSystem, Mailbox, Dispatcher, Supervisor, EventStream, Deadletter, Ask/Future（いずれも `ActorSystem` エイリアスを前提とする）
- 非対象: Typed Behavior 層、クラスタリング、永続化

## シナリオ × テストマトリクス
| シナリオ | 主要要素 | 既存テスト | ギャップ | 新規テスト案 |
| --- | --- | --- | --- | --- |
| US1-1 Ping/Pong | spawn/tell/downcast/reply_to | `ping_pong.rs::spawn_and_tell_delivers_message` | `reply_to` Ask 経路未検証 | ・`ping_pong.rs` に Ask/Reply と `AnyMessage::with_reply_to` を利用したケースを追加 |
| US1-2 メールボックス FIFO/32件 | Mailbox throughput/backpressure | `tell_respects_mailbox_backpressure` | throughput 300 既定値の検証がない | ・`mailbox_throughput.rs` で 300 件目までの処理時間と SystemMessage 優先度を検証 |
| US1-3 Inline Dispatcher | Dispatcher scheduling | 個別テストなし | ・InlineExecutor 駆動で `DispatcherState` の Idle→Running 遷移と `schedule()` 呼び出しを検証 |
| US2-1 Restart | SupervisorStrategy, RestartStatistics | `recoverable_failure_restarts_child` | 最大再起動回数や遅延計測が未検証 | ・再起動カウンタを可視化するヘルパーを追加し、`RestartStatistics::fail` の窓口をアサート |
| US2-2 Escalate | Escalation, Deadletter | `escalate_failure_restarts_supervisor` | 親への Deadletter 通知未検証 | ・エスカレーション時に `system.deadletters()` にエントリが増えることを確認 |
| US3-1 Behavior 遷移ログ | LifecycleEvent (Started/Restarted/Stopped) | `system_events.rs` | `Behavior::become` 未導入のため代替イベント必要 | ・`LifecycleEvent` の `timestamp` 単調性と PID/Parent の整合性を検証 |
| US3-2 Deadletter 転送 | Deadletter, EventStreamSubscriber | `event_stream.rs` | Suspension/Full 以外の理由未検証 | ・宛先不明 PID, Missing reply_to を生成し `DeadletterReason` 別のエントリを観測 |

## 実装/検証方針
1. **テスト補助ヘルパー**: `tests/common.rs` に以下を集約
   - `wait_until(|| ...)`（std/no_std 双方で利用可能な busy loop）
   - `spawn_system(props)` のラッパー（`ActorSystem` を返す軽量ヘルパー）
   - `TestProbe` 風アクター（`reply_to` を活用して受信を記録）
2. **Inline Dispatcher 経路**: 受入テストでは `InlineExecutor` のみを利用し、`DispatcherState` の遷移と Mailbox 計測イベントを同期的にアサートする。
3. **Deadletter/Log 検証**: `EventStreamSubscriber` をテストコード内で実装し、ArcShared<NoStdMutex<Vec<EventStreamEvent>>> に push して期待シーケンスを比較する。
4. **Ask/Future**: `AskResponse::future().listener()` を使い、`poll_immediate` と `await` 双方の API をテストする。完了後に `system.drain_ready_ask_futures()` が空になることも確認する。
5. **Throughput**: Mailbox instrumentation (`MailboxInstrumentation::publish`) に記録されるメトリクスを `EventStreamEvent::Mailbox` で取得し、しきい値超過で WARN ログになることを確認する。

## オープンな課題
- `Behavior::become` が未実装のため、US3-1 の「状態遷移イベント」は Lifecycle イベントで代替する必要がある。仕様変更が必要なら別途議論。
- no_std での長時間 busy wait を避けるため、将来的には `MailboxOfferFuture` を直接 poll する async テストが望ましいが、現行 CI で async executors を持たないためブロッキング helper で代用する。
