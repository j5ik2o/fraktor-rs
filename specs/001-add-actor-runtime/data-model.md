# Data Model: セルアクター no_std ランタイム初期版

## 1. エンティティ概要

| エンティティ | 役割 | 主なフィールド | 関係 |
|--------------|------|----------------|------|
| `ActorSystem` | PID / 名前レジストリと Supervisor ツリーのルート。EventStream/Deadletter を仲介。 | `registry: HashMap<Pid, ActorCellRef>`, `name_registry: HashMap<Name, Pid>`, `event_stream`, `deadletter`, `supervisor_tree`, `scheduler` | `ActorSystem` 1 : N `ActorCell`、1 : 1 `EventStream`、1 : 1 `Deadletter` |
| `ActorCell` | 個々のアクター状態と Mailbox を保持。 | `props`, `mailbox`, `receive_state: ReceiveState`, `supervisor_ref`, `parent: Option<Pid>`, `children: ArrayVec<Pid>`, `lifecycle_state`, `pre_start_done` | `ActorCell` 1 : 1 `Mailbox`、1 : 1 `SupervisorRef`、1 : N 子アクター |
| `ActorContext<'a>` | アクター実行時の API。 | `system: &'a ActorSystem`, `self_pid: &'a Pid`, `current_state: ReceiveState` | `ActorContext` -> `ActorSystem`, `ActorContext` -> `MessageInvoker` |
| `Actor` | 開発者が実装するアクター本体。 | 実装側で定義したフィールド、`ArcShared<dyn Actor>` として保持 | `Actor` <- `Props.factory`, `Actor` -> `ActorContext` |
| `AnyMessage` | Mailbox に格納される所有メッセージ。 | `payload: ArcShared<dyn Any>`, `type_id`, `metadata`, `reply_to: Option<ActorRef>` | `AnyMessage` -> `AnyMessageView`, `AnyMessage` -> `ActorFuture` |
| `AnyMessageView<'a>` | 未型付けメッセージコンテナ（借用ベース）。 | `payload: &'a dyn Any`, `type_id`, `metadata`, `reply_to: Option<&'a ActorRef>` | `AnyMessageView` は `ActorContext`/`MessageInvoker` から渡される |
| `Props` | アクター生成時の設定。 | `factory: fn(&ActorContext) -> impl Actor`, `mailbox_config`, `supervisor_strategy` | `Props` -> `SupervisorStrategy`, `Props` -> `MailboxConfig` |
| `SupervisorStrategy` | 再起動ポリシー。 | `kind: OneForOne/AllForOne`, `max_restarts`, `reset_interval`, `decider: fn(ActorError) -> Decision` | `SupervisorStrategy` -> `ActorError`, `SupervisorStrategy` -> `ActorCell` |
| `ActorError` | ハンドラ戻り値で使用する分類。 | `Recoverable(code)`, `Fatal(code)` | `ActorError` を `SupervisorStrategy` と `Deadletter` が参照 |
| `Mailbox` | AsyncQueue を用いたメッセージキュー。 | `system_queue: AsyncMpscQueue<SystemMessage>`, `user_queue: AsyncMpscQueue<AnyMessage>`, `policy: MailboxPolicy`, `capacity_strategy: Bounded/Unbounded`, `throughput_limit`, `status`, `dispatcher_ref` | `Mailbox` -> `Dispatcher`, `Mailbox` -> `ActorCell` |
| `Dispatcher` | メッセージ処理のスケジューラ。 | `strategy: Immediate/Deferred`, `executor_ref`, `metrics` | `Dispatcher` -> `MessageInvoker`、`Dispatcher` -> `Mailbox` |
| `MessageInvoker` | Mailbox から取り出したメッセージをアクターに渡す実行器。 | `behavior_runner`, `panic_handler`, `middleware_chain: Vec<Middleware>` | `MessageInvoker` -> `ActorContext`, `MessageInvoker` -> `AnyMessage` |
| `EventStream` | 状態遷移・Deadletter・LogEvent 通知。 | `subscribers: [SubscriberHandle; N]`, `buffer` | `EventStream` <- `ActorSystem`, `EventStream` -> `Subscriber` |
| `Deadletter` | 配信失敗メッセージの蓄積。 | `entries: BoundedQueue<DeadletterEntry>`, `event_stream_ref` | `Deadletter` -> `EventStream` |
| `Pid` | アクター識別子。 | `value: u64`, `generation: u32`, `name: Option<Name>` | `Pid` -> `ActorSystem.registry`, `Name` -> `Pid` 逆引き |
| `LogEvent` | Logger 購読者向けイベント。 | `level: LogLevel`, `pid: Option<Pid>`, `message: &'a str`, `timestamp: Instant`, `metadata` | `LogEvent` は `EventStream` から Logger に配送 |

## 2. ライフサイクル / 状態遷移

### ActorCell 状態遷移

```
Created -> Initializing -> Running -> {Suspended, Stopped}
Suspended -> Running (restart成功)
Running -> Stopped (正常停止/ActorError::Fatal)
Running -> Stopped (panic! -> 即時停止、Deadletter通知)
```

- `request.reply_to: ActorRef` で応答先を指定し、ActorContext は送信元を保持しない。

### Supervisor 戦略と子アクター管理

- `Recoverable` : restart(対象=子アクター) / AllForOne の場合は兄弟も再起動、Deadletter に原因を記録。  
- `Fatal` : 子アクターを停止し、EventStream に Fatal 事象を Publish。  
- `panic!` : ランタイムは介入せず、停止イベントを記録後、外部ウォッチドッグ等で復帰。  
- 親アクターは `Context::spawn_child` で子を生成し、`children` リストへ登録。停止時は EventStream に `ChildTerminated` を publish。  
- `pre_start` は ActorCell 初期化後に 1 度のみ呼ばれ、`post_stop` は停止時に必ず呼ばれる。`pre_start_done` フラグで多重実行を防ぐ。

### Mailbox と背圧制御

- 内部は System / User の 2 本の `AsyncMpscQueue<AnyMessage>` で構成され、System キューが空のときのみ User キューを処理する。  
- Bounded 戦略では `MailboxPolicy` に従い DropNewest/DropOldest/Grow/Block を適用する。Grow はバッファ容量を拡張し、Block は WaitNode ベースで待機し、`resume()` または dequeue により通知して解除する。  
- Unbounded 戦略ではメモリ使用量を計測し、しきい値超過時に EventStream/Logger へ Warning を送る。`suspend()` は dequeue を停止し、`resume()` が発火すると再開する。

### Request / Reply フロー

- `AnyMessage` は `reply_to: Option<ActorRef>` を保持して enqueue され、MessageInvoker が借用型 `AnyMessageView` を生成してアクターへ渡す。  
- アクターが `reply_to` を利用して返信するか、`ActorFuture::complete()` を呼ぶことで ask を完了させる。  
- `ActorFuture` レジストリは ActorSystem にあり、完了後は Future を解決して待機している呼び出しに結果を返す。

### Middleware チェーン

- `Middleware` は `before_invoke(ctx, msg)` / `after_invoke(ctx, msg, result)` を持つトレイト。
- 初期リリースでは空チェーンだが、拡張時に挿入可能な配列/スライスで保持する。
- ミドルウェアは System/User 優先度や Logger と独立して順序適用される。

## 3. バリデーションルール

- `Props.mailbox_config.capacity` は 64 メッセージ以上。バックプレッシャー閾値は capacity×0.75。  
- `SupervisorStrategy.max_restarts` は `reset_interval` 内で 10 回までを推奨。超過で子アクター停止。  
- `AnyMessage::downcast::<T>()` 失敗時は `ActorError::Fatal(UnknownMessage)` を返す。  
- `EventStream` の購読者は最大 8 件（`heapless::Vec` 制約）。
- Logger 購読者は最低 1 件。`LogEvent` は UART/RTT 出力またはホスト向けブリッジで扱えるようフォーマットを固定。
- NameRegistry は親スコープごとに名前の一意性を保証し、自動命名は `anon-{pid}` プレフィックスで生成。
- Bounded 戦略では capacity >= 16、Unbounded 戦略ではメモリ水位を監視して閾値超過時に警告イベントを生成。  
- `throughput_limit` の既定値は 300 メッセージ/インボーク。0 で無制限を示し、上限到達時は残りメッセージを次サイクルに繰り越す。

## 4. 関係図（テキスト）

- `ActorSystem` → `ActorCell`（コンポジション）  
- `ActorCell` → `Mailbox`（所有）  
- `Mailbox` ↔ `Dispatcher`（連携）  
- `Dispatcher` → `MessageInvoker` → `ActorContext` → `Actor`  
- `Actor` ← `AnyMessage`（借用経由）  
- `ActorSystem` → `EventStream` / `Deadletter`（通知）  
- `EventStream` → `LoggerSubscriber`（LogEvent 配信）  
- `ActorCell(parent)` → `ActorCell(child)`（階層関係）

## 5. データサイズと計測

- `AnyMessage` はポインタ + TypeId + メタデータ（8+8+?）程度で 32 bytes 以内を目標。  
- `ActorCell` は Mailbox 参照 + Props + 状態で 64 bytes 以内。  
- ヒープ確保箇所は Mailbox のバッファリサイズ（既定では発生しない）と外部プラグインのみ許容。SC-005 のしきい値は研究結果参照。
