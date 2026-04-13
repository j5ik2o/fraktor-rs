## ADDED Requirements

### Requirement: actor runtime の shared wrapper / shared state は builtin spin driver を直接指定して構築されなければならない

actor runtime が管理する shared wrapper / shared state は、型別 `*SharedFactory` や lock factory seam を介さず、`SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` を直接使って構築されなければならない（MUST）。runtime wiring が actor-system scoped な shared factory trait object に依存してはならない（MUST NOT）。

#### Scenario: `SharedLock` ベースの wrapper は direct builtin spin construction で作られる
- **WHEN** runtime wiring が `MessageDispatcherShared`、`ActorRefSenderShared`、`ActorShared`、`ActorCellStateShared`、`ReceiveTimeoutStateShared`、priority queue state shared を構築する
- **THEN** call site または対象型の局所 helper は `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` を使う
- **AND** `MessageDispatcherSharedFactory` や `ActorRefSenderSharedFactory` のような型別 factory trait method を呼ばない

#### Scenario: 複数 lock を持つ型も direct builtin spin construction を使う
- **WHEN** runtime wiring が `ExecutorShared` または `MailboxSharedSet` のように複数 lock を必要とする型を構築する
- **THEN** その構築は必要な lock ごとに `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` を使う
- **AND** call site は actor-system scoped な shared factory seam を経由しない

#### Scenario: `SharedRwLock` ベースの wrapper も direct builtin spin construction に戻る
- **WHEN** runtime wiring が `EventStreamShared`、`MessageInvokerShared`、`DeadLetterShared`、`SystemStateShared` のように `SharedRwLock` を必要とする wrapper を構築する
- **THEN** call site または対象型の局所 helper は `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` を使う
- **AND** `SharedRwLock` 用の別 seam や型別 factory trait を介さない

### Requirement: actor runtime は actor-system scoped な `*SharedFactory` Port を公開契約として残してはならない

actor runtime の shared wrapper / shared state 構築は、公開 contract として actor-system scoped な `*SharedFactory` Port を残してはならない（MUST NOT）。production wiring と公開 API の両方で、direct builtin spin construction が唯一の shared wrapper 構築経路でなければならない（MUST）。

#### Scenario: runtime 公開面に actor-system scoped な `*SharedFactory` trait が存在しない
- **WHEN** actor runtime の公開 trait / module を確認する
- **THEN** `MessageDispatcherSharedFactory`、`ExecutorSharedFactory`、`SharedMessageQueueFactory`、`ActorRefSenderSharedFactory`、`ActorSharedFactory`、`ActorCellStateSharedFactory`、`ReceiveTimeoutStateSharedFactory`、`MessageInvokerSharedFactory`、`ActorFutureSharedFactory<AskResult>`、`TickDriverControlSharedFactory`、`ActorRefProviderHandleSharedFactory<LocalActorRefProvider>`、`EventStreamSharedFactory`、`EventStreamSubscriberSharedFactory`、`MailboxSharedSetFactory`、`ContextPipeWakerHandleSharedFactory`、`BoundedPriorityMessageQueueStateSharedFactory`、`BoundedStablePriorityMessageQueueStateSharedFactory`、`UnboundedPriorityMessageQueueStateSharedFactory` は存在しない
- **AND** shared wrapper / shared state 構築の actor-system override seam は存在しない
