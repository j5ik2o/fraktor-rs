# ActorSystem ガイド

セルアクターランタイムの `ActorSystem` を利用する際の基本手順と、`reply_to` パターンや監視機能の運用ポイントをまとめます。no_std 環境と標準環境（Tokio 連携）で共通する設計指針を把握し、アプリケーションから安全に制御できるようにすることが目的です。

```rust
use fraktor_actor_core_rs::{ActorSystem, ActorSystemGeneric, Props};
use fraktor_actor_std_rs::{StdActorSystem, StdToolbox};
```

## 1. 初期化フロー

- **ユーザガーディアンの定義**: `Props::from_fn(|| GuardianActor)` のようにガーディアンを構築し、no_std 環境では `ActorSystem::new(&guardian_props)`、標準環境では `StdActorSystem::new(&guardian_props)` に渡します。ガーディアンはアプリケーションのエントリポイントであり、`spawn_child` を通じて子アクターを組み立てます。
- **起動メッセージ**: `system.user_guardian_ref().tell(AnyMessage::new(Start))?;` でアプリケーションを起動します。トップレベルのアクター生成はガーディアン（またはその子）経由に限定されます。
- **Mailbox / Dispatcher 構成**: `Props::with_mailbox_strategy` や `Props::with_throughput` を利用して、容量・背圧・スループットの設定を事前に行います。Bounded 戦略では容量 64 以上を推奨し、容量超過ポリシー（DropOldest など）を選択します。

```rust
let guardian_props: Props<StdToolbox> = Props::from_fn(|| GuardianActor)
  .with_mailbox_strategy(MailboxStrategy::bounded(MailboxCapacity::new(64)))
  .with_throughput(300);
let system = StdActorSystem::new(&guardian_props)?;
```

## 2. メッセージ送信と `reply_to` パターン

- ランタイムは Classic の `sender()` を提供しないため、返信が必要な場合は payload に `reply_to: ActorRef` を含めます。
- 送信側は `ctx.self_ref()` などを渡し、受信側が `reply_to.tell(...)` で応答します。
- `ask` を利用する場合は `ActorFuture` を介して待機できます。Guardian など制御側で `system.drain_ready_ask_futures()` を定期的に呼び、完了した Future を回収します。

```rust
struct StartPing {
  target:   ActorRef,
  reply_to: ActorRef,
  count:    usize,
}

ping.tell(AnyMessage::new(StartPing { target: pong, reply_to: ctx.self_ref(), count: 3 }))?;
```

## 3. 監督機能と停止フロー

- アクターは `pre_start` → `receive` → `post_stop` のライフサイクルを持ち、`ActorError::Recoverable` で再起動、`ActorError::Fatal` で停止します。
- `ctx.stop_self()` や `system.terminate()` を呼ぶと、ユーザガーディアンに `SystemMessage::Stop` が送られ、子アクターへ停止が伝播します。
- ランタイム終了待機には `system.when_terminated()` を利用し、同期環境では `run_until_terminated()`、非同期環境では `await` で待機します。

```rust
let termination = system.when_terminated();
system.terminate()?;
while !termination.is_ready() {
  core::hint::spin_loop();
}
```

## 4. 監視とオブザーバビリティ

- **EventStream**: ライフサイクル・ログ・Deadletter を publish するバスです。`system.subscribe_event_stream(subscriber)` で購読し、`on_event` で各種イベントを処理します。既定バッファ容量は 256 件で、超過すると最古のイベントから破棄されます。
- **Deadletter**: 未配達メッセージを 512 件保持し、登録時に `EventStreamEvent::Deadletter` と `LogEvent` を発火します。容量変更が必要な場合は今後追加予定の `actor-std` ヘルパー（ActorSystemConfig 仮称）での設定を検討します。
- **LoggerSubscriber**: `LogLevel` フィルタ付きで EventStream を購読し、UART/RTT やホストログへ転送します。Deadletter が 75% に達したなどの警告閾値を購読者側で判断し、任意の通知手段へ連携してください。

```rust
let logger = ArcShared::new(LoggerSubscriber::new(LogLevel::Info, ArcShared::new(MyWriter)));
let _subscription = system.subscribe_event_stream(logger);
```

## 5. Tokio ランタイムとの連携

- `modules/actor-core/examples/ping_pong_tokio` では、Tokio マルチスレッドランタイム上で Dispatcher を駆動するサンプルを確認できます。
- `DispatcherConfig::<StdToolbox>::from_executor(ArcShared::new(TokioExecutor::new(handle)))` を利用し、`Handle::spawn_blocking` 上で `dispatcher.drive()` を実行します。これにより `async fn` へ依存せずランタイム外部のスレッドプールでメッセージ処理を行えます。
- 今後 `actor-std` クレートへ追加する拡張 API（例: Tokio ランタイムハンドルからの安全な取得）については、本ガイドと quickstart を同時に更新し、no_std な `actor-core` への追加依存が発生しないようにします。

## 6. トラブルシュートのヒント

- **Mailbox が溢れる**: capacity を増やすか、`SendError::Full` を受け取ったときにバックオフ処理を挟みます。Deadletter の Warn ログを監視し、容量やポリシーを再調整してください。
- **返信が届かない**: payload に `reply_to` が含まれているか、メッセージ型を `downcast_ref::<T>()` で正しく解釈しているかを確認します。
- **Tokio 連携で停止しない**: `system.when_terminated()` を `await` し忘れているか、ガーディアンが自己停止していない可能性があります。`system.terminate()` 後に `when_terminated()` の完了を待機してください。

以上の手順と注意点を押さえておけば、組込みからホスト環境まで一貫した ActorSystem の運用と監視が可能になります。今後 `actor-std` にヘルパー API が追加された際は、本ガイドと quickstart を更新し、想定されるブートストラップ手順を最新化してください。
