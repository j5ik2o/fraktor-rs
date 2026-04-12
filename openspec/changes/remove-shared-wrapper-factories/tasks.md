## 1. Direct builtin spin construction への回帰

- [x] 1.1 `MessageDispatcherShared`、`ExecutorShared`、`ActorRefSenderShared`、`ActorShared`、`ActorCellStateShared`、`ReceiveTimeoutStateShared`、priority queue state shared の構築を `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` ベースへ戻す
- [x] 1.2 `EventStreamShared`、`MessageInvokerShared`、`DeadLetterShared`、`SystemStateShared` など `SharedRwLock` 関連 wrapper の構築を `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` ベースへ戻す

## 2. Runtime wiring と config の単純化

- [x] 2.1 `DefaultDispatcherConfigurator`、`BalancingDispatcherConfigurator`、`PinnedDispatcherConfigurator`、spawn path、bootstrap、tick driver、event stream helper、mailbox helper から `*SharedFactory` 依存を除去する
- [x] 2.2 `ActorSystemConfig` / `ActorSystemSetup` から shared runtime override field / API（`with_shared_factory(...)` を含む）を削除し、default builtin spin 構成へ整理する

## 3. 旧 strategy の削除

- [ ] 3.1 `ActorLockFactory` trait と、`MessageDispatcherSharedFactory`、`ExecutorSharedFactory`、`SharedMessageQueueFactory`、`ActorRefSenderSharedFactory`、`ActorSharedFactory`、`ActorCellStateSharedFactory`、`ReceiveTimeoutStateSharedFactory`、`MessageInvokerSharedFactory`、`ActorFutureSharedFactory<AskResult>`、`TickDriverControlSharedFactory`、`ActorRefProviderHandleSharedFactory<LocalActorRefProvider>`、`EventStreamSharedFactory`、`EventStreamSubscriberSharedFactory`、`MailboxSharedSetFactory`、`ContextPipeWakerHandleSharedFactory`、`BoundedPriorityMessageQueueStateSharedFactory`、`BoundedStablePriorityMessageQueueStateSharedFactory`、`UnboundedPriorityMessageQueueStateSharedFactory` を削除する
- [ ] 3.2 `modules/actor-adaptor-std/src/std/system/shared_factory/`、`StdActorSharedFactory`、`DebugActorSharedFactory`、`BuiltinSpinSharedFactory` と、それらに依存する公開面・wiring・tests を削除または置換する
- [ ] 3.3 `ActorFutureSharedFactory<AskResult>` は direct construction へ吸収し、`CircuitBreakerSharedFactory<C>` は type-indexed registry として据え置く境界をコード上で明確にする

## 4. テストと OpenSpec の整合

- [ ] 4.1 `with_shared_factory(...)` の既存 16 call site と `std/system/shared_factory/tests.rs`、`actor_system_config/tests.rs`、`typed/system/tests.rs`、dispatcher tests 群を direct builtin spin construction 前提へ置換する
- [ ] 4.2 dispatcher configurator の instance 戦略、`SharedRwLock` wrapper の direct builtin spin construction、config override API の削除、std 公開面の削除を確認するテストと OpenSpec 整合を更新する
- [ ] 4.3 `serialization_registry` など actor runtime core path か境界判断が必要な `SharedRwLock` 利用箇所は、この change に含めるか follow-up へ分離するかを実装中に明示する
