# Capability: Shared Mailbox Runtime Abstractions

ランタイム非依存のメールボックス抽象、および Block ポリシー対応の独自 Future を備えたキュー基盤を定義し、全く新規のコードベースでも一貫した挙動を実現する。

## ADDED Requirements

### Requirement: Shared mailbox module exposes cohesive public surface
- ルートモジュール `shared::mailbox` は `consumer`, `factory`, `messages`, `options`, `producer`, `signal` サブモジュールを宣言しなければならない。
- `consumer`, `factory`, `options`, `producer`, `signal` の公開アイテムは `pub use` を通じてルートから再エクスポートされ、`messages` はネストモジュールとして公開されなければならない。

#### Scenario: Downstream module imports mailbox traits once

Given 利用側が use shared::mailbox::*; のみでメールボックス API を取り込みたい
When コンパイルを行う
Then consumer/factory/options/producer/signal の要素を追加のパス指定なしに利用できる


### Requirement: MailboxConsumer trait integrates mailbox behavior and signaling
- `MailboxConsumer<M>` は `Mailbox<M>` と `Clone` を継承し、`MailboxSignal` を実装する関連型 `Signal` を持たなければならない。
- `signal(&self) -> Signal`, `try_dequeue(&self) -> Result<Option<M>, QueueError<M>>`, `try_dequeue_mailbox(&self) -> Result<Option<M>, MailboxError<M>>` を提供し、後者は前者の結果を mailbox エラーに変換しなければならない。

#### Scenario: Scheduler performs non-blocking dequeue

Given 待機中メッセージを保持する MailboxConsumer がある
When ランタイムが try_dequeue() を呼び出す
Then 待機せずに Ok(Some(_)) を返す
And キュー切断時には Err(QueueError::Disconnected) を返す


### Requirement: MailboxFactory produces paired consumer and producer with options
- `MailboxFactory` は `Concurrency`, `Signal`, `Queue<M>`, `Mailbox<M>`, `Producer<M>` の関連型を定義し、それぞれ既存抽象（Concurrency: MailboxConcurrency + MetadataStorageMode, Queue<M>: MailboxQueueBackend, Mailbox<M>:
  MailboxConsumer, Producer<M>: MailboxProducer）を満たさなければならない。
- `build_mailbox(options)` は `(Mailbox, Producer)` を返し、容量などの設定は `MailboxOptions` で制御しなければならない。
- `build_default_mailbox()` は `MailboxOptions::default()` を渡して `build_mailbox` を再利用しなければならない。

#### Scenario: Factory respects custom capacity

Given MailboxOptions::with_capacity(128) を渡す
When build_mailbox(options) を呼ぶ
Then 返された Mailbox は容量制限 128 を守る
And 同じ Mailbox と連携する Producer が返る


### Requirement: PriorityEnvelope preserves message metadata
- `PriorityEnvelope<M>` はメッセージ本体、優先度 (i8)、配送チャネル (`PriorityChannel`)、任意のシステムメッセージ参照を保持しなければならない。
- `new`, `with_channel`, `control`, `with_default_priority` は指定されたチャネル・優先度を設定し、`map`, `map_priority`, `into_parts` 派生 API を備えなければならない。
- `PriorityEnvelope<SystemMessage>::from_system` は `SystemMessage` をクローンし、チャネルを `Control` に設定し、元メッセージの優先度を保持したまま `system_message` フィールドへ格納しなければならない。
- `PriorityMessage` 実装により `get_priority` で優先度を返し、`M` が `Send`/`Sync` の場合は `PriorityEnvelope<M>` も `Send`/`Sync` を満たさなければならない。

#### Scenario: Control message keeps metadata intact

Given priority 42 の SystemMessage がある
When PriorityEnvelope::from_system(message) を呼ぶ
Then channel() は PriorityChannel::Control を返す
And system_message().unwrap().priority() は 42 を返す


### Requirement: MailboxOptions expresses queue capacities with defaults
- `MailboxOptions` は `QueueSize` を用いた `capacity` と `priority_capacity` を保持しなければならない。
- 既定値は通常メッセージ無制限、優先度キューは `DEFAULT_SYSTEM_RESERVATION = 4` に設定しなければならない。
- `with_capacity`, `with_capacities`, `with_priority_capacity`, `unbounded` は内部 `QueueSize` を適切に構成し、`capacity_limit`, `priority_capacity_limit` は有限値なら `Some(limit)`, 無制限なら `None` を返さなければならない。

#### Scenario: Default reservation applied

Given MailboxOptions::default() を生成した
When priority_capacity_limit() を呼ぶ
Then Some(4) を返す
And capacity_limit() は None を返す


### Requirement: MailboxProducer trait handles enqueue instrumentation hooks
- `MailboxProducer<M>` は `Clone` であり、`try_send(&self, M) -> Result<(), QueueError<M>>` と `try_send_mailbox(&self, M) -> Result<(), MailboxError<M>>` を提供しなければならない。
- メトリクス・スケジューラ用フック `set_metrics_sink`, `set_scheduler_hook` を持ち、既定では no-op を維持しなければならない。

#### Scenario: Producer reports backpressure via mailbox error

Given 満杯状態の MailboxProducer がある
When try_send_mailbox(message) を呼ぶ
Then Err(MailboxError::Queue(QueueError::Full(message))) を返す


### Requirement: MailboxSignal trait models asynchronous notifications
- `MailboxSignal` は `Clone` を実装し、`Future<Output = ()>` を満たす関連型 `WaitFuture<'a>` を定義しなければならない。
- `notify(&self)` は待機者を起床させ、`wait(&self)` は通知後に完了する Future を返さなければならない。

#### Scenario: Receiver waits until notification

Given 未通知状態の MailboxSignal 実装がある
When wait() を await し、別タスクで notify() を呼ぶ
Then wait() は notify() の後に完了する


### Requirement: Mailbox queue backend exposes custom Futures for blocking policies
- `MailboxQueueBackend<M>` は関連型 `OfferFuture<'a, M>` と `PollFuture<'a, M>`（ともに `Future<Output = Result<..., QueueError<M>>>`）を定義し、`fn offer_blocking(&self, message: M) -> Self::OfferFuture<'_, M>` と `fn
  poll_blocking(&self) -> Self::PollFuture<'_, M>` を提供しなければならない。
- `OfferFuture` は内部でメッセージ所有権と待機ハンドルを保持し、キューが満杯の場合に非同期 WaitQueue へ登録して `Poll::Pending` を返し、空きが発生した際に再試行して `Poll::Ready` を返さなければならない。
- `PollFuture` はキューが空の場合に WaitQueue へ登録し、メッセージが届いた時点で `Poll::Ready(Some(_))` を返さなければならない。
- 既存の非ブロッキング API (`try_offer` / `try_poll` など) は後方互換のため維持し、Block ポリシーは `offer_blocking` / `poll_blocking` を経由しなければならない。

#### Scenario: OfferFuture suspends until capacity available

Given 容量 2 の QueueBackend に既に 2 件メッセージがある
When offer_blocking(third_message) の Future を poll する
Then 初回 poll で WaitQueue に登録され Poll::Pending を返す
And コンシューマが 1 件取り出した後に再度 poll すると Poll::Ready(Ok(_)) を返す


### Requirement: Queue mailbox core consumes custom Futures for Block policy
- `QueueMailboxCore` および関連ハンドルは Block ポリシー時に `offer_blocking` / `poll_blocking` を用い、結果を待つ間はランタイムの waker に制御を返さなければならない。
- 非 Block ポリシー（Drop/Grow 等）は従来どおり即時応答で動作し、Block 専用 Future を使わないこと。
- ランタイム非依存を保つため、Future 実装は標準ライブラリの `core::task` API のみで完結しなければならない。

#### Scenario: Mailbox with Block policy resumes producer after dequeue

Given Block ポリシーの QueueMailbox に producer が 3 件目を送信しようとしている
When consumer が poll_blocking() を await し、1 件 dequeue する
Then 待機していた producer 側の offer_blocking() Future が目覚め、送信が完了する


### Requirement: WaitQueue integration avoids async fn usage
- Block 走査で利用する Future 実装は `async fn` を用いず、`poll` メソッド内で `AsyncQueue` または同等の待機基盤へ直接アクセスしなければならない。
- Future が `Poll::Pending` を返す直前に、所有するガードやロックを開放しデッドロックを防止しなければならない。
- Future が drop された場合は WaitQueue からの登録解除を確実に行うこと。

#### Scenario: Dropped Future releases wait registration

Given offer_blocking() の Future が WaitQueue 登録後に drop された
When その後にキューへ空きができる
Then WaitQueue は既に解除済みであり不要な通知やハンドルリークが発生しない
