## Why

`*SharedFactory` 戦略は lock family 差し替えを意図して導入されたが、実際には `BuiltinSpinSharedFactory` / `StdActorSharedFactory` / `DebugActorSharedFactory` が同型の `create_*` 実装を大量に持つだけになっている。pre-release フェーズで後方互換も不要なので、actor runtime の shared wrapper / shared state 構築は `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` と `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` に戻し、構造コストを削る。

## What Changes

- actor runtime の shared wrapper / shared state 構築を、型別 `*SharedFactory` 経由ではなく builtin spin driver 直指定へ戻す
- dispatcher configurator、spawn path、bootstrap、shared queue / mailbox bundle / event stream / message invoker などの wiring を `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` または `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` ベースへ戻す
- `ActorSystemConfig` / `ActorSystemSetup` から shared-factory override API と関連 field を削除し、shared runtime surface は builtin spin backend 固定とする
- `modules/actor-adaptor-std/src/std/system/shared_factory/` と `StdActorSharedFactory` / `DebugActorSharedFactory` の公開面を削除する
- **BREAKING** actor-system scoped な actor runtime `*SharedFactory` trait と、それに依存する config / wiring / test double / std adapter 公開型を削除する。対象には少なくとも `MessageDispatcherSharedFactory`、`ExecutorSharedFactory`、`SharedMessageQueueFactory`、`ActorRefSenderSharedFactory`、`ActorSharedFactory`、`ActorCellStateSharedFactory`、`ReceiveTimeoutStateSharedFactory`、`MessageInvokerSharedFactory`、`ActorFutureSharedFactory<AskResult>`、`TickDriverControlSharedFactory`、`ActorRefProviderHandleSharedFactory<LocalActorRefProvider>`、`EventStreamSharedFactory`、`EventStreamSubscriberSharedFactory`、`MailboxSharedSetFactory`、`ContextPipeWakerHandleSharedFactory`、`BoundedPriorityMessageQueueStateSharedFactory`、`BoundedStablePriorityMessageQueueStateSharedFactory`、`UnboundedPriorityMessageQueueStateSharedFactory` を含む

## Capabilities

### New Capabilities
- `actor-builtin-spin-shared-construction`: actor runtime の shared wrapper / shared state は builtin spin driver を直接指定して構築される

### Modified Capabilities
- `actor-system-default-config`: actor system は shared runtime override seam を持たず、builtin spin backend で default dispatcher と shared runtime surface を seed する
- `dispatcher-trait-provider-abstraction`: dispatcher / executor / shared queue / actor-ref sender / mailbox lock bundle の構築は型別 factory Port ではなく direct builtin spin construction へ変更される
- `actor-std-adapter-surface`: std adapter 公開面から `shared_factory` module と `StdActorSharedFactory` / `DebugActorSharedFactory` を除外する

## Impact

- 対象コード:
  - `modules/actor-core/src/core/kernel/system/shared_factory/`
  - `modules/actor-core/src/core/kernel/dispatch/`
  - `modules/actor-core/src/core/kernel/actor/setup/`
  - `modules/actor-adaptor-std/src/std/system/shared_factory/`
- 影響内容:
  - `ActorSystemConfig` / `ActorSystemSetup` の shared-factory override API と保持 field が削除される
  - dispatcher configurator や spawn/bootstrap path は `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` / `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` を直接使う構造へ戻る
  - std adapter の shared factory 公開型削除に伴い、利用側の設定コードは default builtin spin 構成へ寄る
- 非目標:
  - debug/std lock family 切替を別の abstraction で維持すること
  - `CircuitBreakerSharedFactory<C>` のような type-indexed registry をこの change で撤去すること
  - actor runtime 以外の module に同じ方針を広げること
