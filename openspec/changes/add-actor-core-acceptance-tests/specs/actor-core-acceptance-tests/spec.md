## ADDED Requirements

### Requirement: actor-core 受入テストカバレッジ
`modules/actor-core/tests` SHALL `specs/001-add-actor-runtime` の US1〜US3 を自動テストとして再現し、`ActorSystem`（= `ActorSystemGeneric<NoStdToolbox>`）構成で実行可能にしなければならない。

#### Scenario: US1 最小アクターの spawn/tell/ask を検証する
- **GIVEN** `ActorSystem::new` でユーザガーディアンを起動し、`Props` から子アクターを `spawn_child` できるフィクスチャが用意されている
- **WHEN** テストが `AnyMessage::new(Ping).with_reply_to(...)` を使って `tell` と `ask` の両経路を実行し、`ActorFuture::listener()` で返信を待機する
- **THEN** `ctx.self_ref()` 経由で取得した `ActorRef` が `reply_to` に設定され、`ActorFuture` が完了して `Pong` を受け取るまでに Deadletter が発生しないこと、並びに downcast (`AnyMessageView::downcast_ref::<Ping>()`) が成功することがアサートされる

#### Scenario: US1 メールボックスの FIFO・スループット・バックプレッシャーを検証する
- **GIVEN** Mailbox policy が `NonZeroUsize::new(32)` の bounded 設定および `throughput_limit = Some(NonZeroUsize::new(300))` を持つ
- **WHEN** テストが 32 件超のユーザメッセージと System メッセージを enqueue し、`MailboxInstrumentation` が発火するまで `Dispatcher::drive` を繰り返す
- **THEN** System キューが常に優先され、33 件目以降は `SendError::Full` もしくは `EnqueueOutcome::Pending` になること、300 件処理後には処理が一旦停止して次スケジュールで再開されることを検証する

#### Scenario: US1 Inline Executor で Dispatcher を駆動する
- **GIVEN** テストが `ActorSystem` と `InlineExecutor` の組み合わせで Dispatcher を構築し、`DispatcherState` を直接観測できる instrumentation を有効化している
- **WHEN** テストが複数メッセージを enqueue して `Dispatcher::schedule` を連続呼び出しし、Idle→Running 遷移と `drive()` ループの継続条件を手動で進める
- **THEN** InlineExecutor 上でもスループット上限に達した時点で実行が一旦停止し、次の `schedule()` で再開すること、`DispatcherState` が Idle に戻ることを検証する

#### Scenario: US2 監視戦略・エスカレーションを検証する
- **GIVEN** `SupervisorStrategyKind::OneForOne`／`AllForOne`／`Escalate` を切り替えられるテスト用 Props がある
- **WHEN** Recoverable/Fatal/Panic メッセージを子アクターへ送り、`RestartStatistics` と `system.deadletters()` を観測する
- **THEN** 最大再起動回数や遅延ウィンドウが仕様どおりに更新され、Escalate 時には親の `post_stop` が呼ばれた後に Deadletter が追加されることをアサートする

#### Scenario: US3 EventStream / Deadletter / Lifecycle / Mailbox メトリクスを検証する
- **GIVEN** テストが `EventStreamSubscriber` を実装し、ArcShared<NoStdMutex<Vec<EventStreamEvent>>> へ push する
- **WHEN** アクターの `pre_start`/`receive`/`post_stop`、`ctx.log`、宛先不明 PID、`reply_to` 欠落 ask など複数のイベントを発火させる
- **THEN** `EventStreamEvent::Lifecycle` の PID/parent/timestamp、`EventStreamEvent::Deadletter` の `DeadletterReason`、`EventStreamEvent::Mailbox` のバッファ長、`LogEvent` のメッセージが仕様どおりであることを比較し、購読解除後はイベントが流れてこないことを保証する
