## ADDED Requirements

### Requirement: actor-core 受入テストカバレッジ
`modules/actor-core/tests` および `modules/actor-std/tests` SHALL `specs/001-add-actor-runtime` の US1〜US3 を自動テストとして再現し、TokioExecutor（`DispatcherConfig::from_executor` で注入）を用いた ActorSystem 実行経路でパスしなければならない。

#### Scenario: US1 最小アクターの spawn/tell/ask を検証する（Tokio）
- **GIVEN** `tokio::runtime::Runtime` が初期化され、`DispatcherConfig::from_executor(ArcShared::new(TokioExecutor::new(handle)))` を使って ActorSystem を起動できるフィクスチャがある
- **WHEN** テストが `AnyMessage::new(Ping).with_reply_to(...)` を使って `tell` と `ask` の両経路を実行し、`ActorFuture::listener().await` で返信を待機する
- **THEN** `ctx.self_ref()` が `reply_to` に設定されたまま `Pong` を受け取り、Deadletter が発生しないこと、および Tokio の `spawn_blocking` ログで dispatcher が実行されたことを確認する

#### Scenario: US1 メールボックスの FIFO・スループット・バックプレッシャーを検証する（Tokio）
- **GIVEN** Mailbox policy が `NonZeroUsize::new(32)` の bounded 設定および `throughput_limit = Some(NonZeroUsize::new(300))` を持ち、TokioExecutor が Dispatcher に設定されている
- **WHEN** テストが 32 件超のユーザメッセージと System メッセージを enqueue し、Tokio のタスクを `spawn_blocking` で複数回起動してバッチ処理を継続させる
- **THEN** System キューが常に優先され、33 件目以降は `SendError::Full` もしくは `EnqueueOutcome::Pending` になること、300 件処理後には dispatcher が Idle に戻り次の `schedule()` で再開されることを検証する

#### Scenario: US1 Tokio Executor で Dispatcher を駆動する
- **GIVEN** テストが `TokioExecutor` を利用して dispatcher を構築し、`DispatcherState` を観測できる instrumentation を有効化している
- **WHEN** テストが複数メッセージを enqueue して `Dispatcher::schedule` を連続呼び出しし、Tokio の `spawn_blocking` 内で Idle→Running 遷移をトレースする
- **THEN** スループット上限に達した時点でタスクが一旦停止し、次の `schedule()` で再開すること、`DispatcherState` が Idle に戻ることを検証する

#### Scenario: US2 監視戦略・エスカレーションを検証する
- **GIVEN** `SupervisorStrategyKind::OneForOne`／`AllForOne`／`Escalate` を切り替えられるテスト用 Props がある
- **WHEN** Recoverable/Fatal/Panic メッセージを子アクターへ送り、`RestartStatistics` と `system.deadletters()` を観測する
- **THEN** 最大再起動回数や遅延ウィンドウが仕様どおりに更新され、Escalate 時には親の `post_stop` が呼ばれた後に Deadletter が追加されることをアサートする

#### Scenario: US3 EventStream / Deadletter / Lifecycle / Mailbox メトリクスを検証する
- **GIVEN** テストが `EventStreamSubscriber` を実装し、ArcShared<NoStdMutex<Vec<EventStreamEvent>>> へ push する
- **WHEN** アクターの `pre_start`/`receive`/`post_stop`、`ctx.log`、宛先不明 PID、`reply_to` 欠落 ask など複数のイベントを発火させる
- **THEN** `EventStreamEvent::Lifecycle` の PID/parent/timestamp、`EventStreamEvent::Deadletter` の `DeadletterReason`、`EventStreamEvent::Mailbox` のバッファ長、`LogEvent` のメッセージが仕様どおりであることを比較し、購読解除後はイベントが流れてこないことを保証する
