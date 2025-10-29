# Data Model: セルアクター no_std ランタイム初期版

## 1. エンティティ概要

| エンティティ | 役割 | 主なフィールド | 関係 |
|--------------|------|----------------|------|
| `ActorSystem` | PID レジストリと Supervisor ツリーのルート。EventStream/Deadletter を仲介。 | `registry: HashMap<Pid, ActorCellRef>` (no_std 向け `BTreeMap`/`heapless::FnvIndexMap` を想定), `event_stream`, `deadletter`, `supervisor_tree`, `scheduler` | `ActorSystem` 1 : N `ActorCell`、1 : 1 `EventStream`、1 : 1 `Deadletter` |
| `ActorCell` | 個々のアクター状態と Mailbox を保持。 | `props`, `mailbox`, `behavior: BehaviorState`, `supervisor_ref`, `lifecycle_state` | `ActorCell` 1 : 1 `Mailbox`、1 : 1 `SupervisorRef`、1 : N 子アクター |
| `ActorContext<'a>` | アクター実行時の API。 | `system: &'a ActorSystem`, `self_pid: &'a Pid`, `sender: Option<&'a Pid>`, `current_state: BehaviorState` | `ActorContext` -> `ActorSystem`, `ActorContext` -> `MessageInvoker` |
| `AnyMessage<'a>` | 未型付けメッセージコンテナ（借用ベース）。 | `payload: &'a dyn Any`, `type_id`, `metadata` | `AnyMessage` は `ActorContext`/`MessageInvoker` から渡される |
| `Props` | アクター生成時の設定。 | `factory: fn(&ActorContext) -> impl Actor`, `mailbox_config`, `supervisor_strategy` | `Props` -> `SupervisorStrategy`, `Props` -> `MailboxConfig` |
| `SupervisorStrategy` | 再起動ポリシー。 | `kind: OneForOne/AllForOne`, `max_restarts`, `reset_interval`, `decider: fn(ActorError) -> Decision` | `SupervisorStrategy` -> `ActorError`, `SupervisorStrategy` -> `ActorCell` |
| `ActorError` | ハンドラ戻り値で使用する分類。 | `Recoverable(code)`, `Fatal(code)` | `ActorError` を `SupervisorStrategy` と `Deadletter` が参照 |
| `Mailbox` | AsyncQueue を用いたメッセージキュー。 | `queue: AsyncQueue<AnyOwnedMessage>`, `status`, `dispatcher_ref` | `Mailbox` -> `Dispatcher`, `Mailbox` -> `ActorCell` |
| `Dispatcher` | メッセージ処理のスケジューラ。 | `strategy: Immediate/Deferred`, `executor_ref`, `metrics` | `Dispatcher` -> `MessageInvoker`、`Dispatcher` -> `Mailbox` |
| `MessageInvoker` | Mailbox から取り出したメッセージをアクターに渡す実行器。 | `behavior_runner`, `panic_handler` | `MessageInvoker` -> `ActorContext`, `MessageInvoker` -> `AnyMessage` |
| `EventStream` | 状態遷移・Deadletter 通知。 | `subscribers: [SubscriberHandle; N]`, `buffer` | `EventStream` <- `ActorSystem`, `EventStream` -> `Subscriber` |
| `Deadletter` | 配信失敗メッセージの蓄積。 | `entries: BoundedQueue<DeadletterEntry>`, `event_stream_ref` | `Deadletter` -> `EventStream` |
| `Pid` | アクター識別子。 | `value: u64`, `generation: u32` | `Pid` -> `ActorSystem.registry` |

## 2. ライフサイクル / 状態遷移

### ActorCell 状態遷移

```
Created -> Initializing -> Running -> {Suspended, Stopped}
Suspended -> Running (restart成功)
Running -> Stopped (正常停止/ActorError::Fatal)
Running -> Stopped (panic! -> 即時停止、Deadletter通知)
```

### Supervisor 戦略

- `Recoverable` : restart(対象=子アクター) / AllForOne の場合は兄弟も再起動、Deadletter に原因を記録。  
- `Fatal` : 子アクターを停止し、EventStream に Fatal 事象を Publish。  
- `panic!` : ランタイムは介入せず、停止イベントを記録後、外部ウォッチドッグ等で復帰。

## 3. バリデーションルール

- `Props.mailbox_config.capacity` は 64 メッセージ以上。バックプレッシャー閾値は capacity×0.75。  
- `SupervisorStrategy.max_restarts` は `reset_interval` 内で 10 回までを推奨。超過で子アクター停止。  
- `AnyMessage::downcast::<T>()` 失敗時は `ActorError::Fatal(UnknownMessage)` を返す。  
- `EventStream` の購読者は最大 8 件（`heapless::Vec` 制約）。

## 4. 関係図（テキスト）

- `ActorSystem` → `ActorCell`（コンポジション）  
- `ActorCell` → `Mailbox`（所有）  
- `Mailbox` ↔ `Dispatcher`（連携）  
- `Dispatcher` → `MessageInvoker` → `ActorContext` → `Actor`  
- `Actor` ← `AnyMessage`（借用経由）  
- `ActorSystem` → `EventStream` / `Deadletter`（通知）

## 5. データサイズと計測

- `AnyMessage` はポインタ + TypeId + メタデータ（8+8+?）程度で 32 bytes 以内を目標。  
- `ActorCell` は Mailbox 参照 + Props + 状態で 64 bytes 以内。  
- ヒープ確保箇所は Mailbox のバッファリサイズ（既定では発生しない）と外部プラグインのみ許容。SC-005 のしきい値は研究結果参照。
