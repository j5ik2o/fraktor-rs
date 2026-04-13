## ADDED Requirements

### Requirement: dispatcher wiring は direct builtin spin construction で shared handle を構築しなければならない

dispatcher wiring は `MessageDispatcherSharedFactory` や `SharedMessageQueueFactory` のような型別 factory Port を介さず、direct builtin spin construction で shared handle を構築しなければならない（MUST）。dispatcher configurator と balancing queue 構築が actor-system scoped な factory trait object に依存してはならない（MUST NOT）。

#### Scenario: default と balancing の configurator は eager instance を direct builtin spin construction で構築する
- **WHEN** `DefaultDispatcherConfigurator` または `BalancingDispatcherConfigurator` が初期化される
- **THEN** configurator は `MessageDispatcherShared::from_shared_lock(...)` と `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` を使って eager instance を構築する
- **AND** `BalancingDispatcher` が使う shared queue も builtin spin backend で構築される
- **AND** configurator が `MessageDispatcherSharedFactory` または `SharedMessageQueueFactory` を保持しない

#### Scenario: pinned configurator は direct builtin spin construction で毎回新規 dispatcher を構築する
- **WHEN** `PinnedDispatcherConfigurator::dispatcher(&self)` が呼ばれる
- **THEN** その呼び出しで作られる `MessageDispatcherShared` は direct builtin spin construction で構築される
- **AND** fresh instance 戦略は維持される
- **AND** per-call dispatcher 構築のために型別 shared factory trait を経由しない

#### Scenario: dispatcher wiring は builtin spin driver 名を直接使う
- **WHEN** production path が dispatcher、executor、shared queue を構築する
- **THEN** call site または局所 helper は `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` を使う
- **AND** read/write lock が必要な箇所は `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` を使う
- **AND** backend 選択のための factory seam は介在しない
