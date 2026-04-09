## Why

`RuntimeMutex<T>` / `RuntimeRwLock<T>` は現在も [`SpinSyncMutex<T>` / `SpinSyncRwLock<T>` の alias](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/utils-core/src/core/sync/runtime_lock_alias.rs) に留まっており、lock 実装を差し替える seam が存在しない。そのため、std 環境向けの通常 driver や、再入検知つき debug driver を caller 側から選択できない。

この制約は actor-core の再入 hot path で特に問題になる。現在の send/schedule/run 経路には、inline executor 下で同一スレッド再入が起こり得る箇所があり、コード上も deadlock リスクを強く意識している。

代表例:

- [`ActorRefSenderShared`](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor-core/src/core/kernel/actor/actor_ref/actor_ref_sender_shared.rs) は lock 解放後に schedule outcome を適用して再入 deadlock を避けている
- [`DispatcherSender`](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatcher_sender.rs) は enqueue と schedule を 2 phase に分離している
- [`MessageDispatcherShared::dispatch` / `register_for_execution`](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher_shared.rs) は inline executor で `mailbox.run(...)` が同一スレッド実行されることを前提に warning を持っている
- [`SpinSyncMutex::lock()`](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/utils-core/src/core/sync/spin_sync_mutex.rs) の rustdoc でも、将来的な debug variant 再導入をこの refactoring の後に行うと明記している
- [`utils-adaptor-std`](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/utils-adaptor-std/src/lib.rs) は `DebugSpinSyncMutex` 再導入のための placeholder のまま残っている

したがって Phase V の最小 goal は「workspace 全体 genericization」ではない。まず **actor-core の再入 hot path に限って lock-driver seam を導入し、driver 差し替えと deadlock 検知を可能にする** ことである。

## What Changes

- `utils-core` に `LockDriver` / `RwLockDriver` と corresponding factory seam を導入する
- `RuntimeMutex` / `RuntimeRwLock` を driver 差し替え可能な port surface へ昇格する
- `RuntimeMutex<T>` / `RuntimeRwLock<T>` という名前自体は default-driver surface として維持し、既存 caller が一括書き換えを要求されないようにする
- `utils-core` には no_std builtin driver として `SpinSyncMutex` / `SpinSyncRwLock` を残す
- `utils-adaptor-std` には std adapter driver として以下を追加する
  - `DebugSpinSyncMutex`
  - `DebugSpinSyncRwLock`
  - `StdSyncMutex`
  - `StdSyncRwLock`
- actor-core では再入 hot path に属する shared wrapper / mailbox / dispatcher 周辺を優先して factory genericization する
- public API (`ActorSystem`, typed system, `ActorRef`) は nongeneric のまま維持し、driver family の選択は bootstrap / configurator 境界で固定する

## Recommended Scope

最初の実装対象は actor-core hot path に絞る。

### Phase V-A: hot path seam

- `ActorRefSenderShared`
- `MessageDispatcherShared`
- `ExecutorShared`
- `Mailbox`

### Phase V-B: actor-core 残り shared wrapper

- `SharedMessageQueue`
- `EventStreamShared`
- `ActorRefProviderShared`
- `RemoteWatchHookShared`
- `SchedulerShared`
- `SerializationExtensionShared`

### Phase V-C: 後続 crate

- `cluster-core`
- `persistence-core`
- `stream-core`
- その他 actor-core 外の wrapper

## Non-Goals

- workspace 全体の lock 利用箇所を一括 genericization すること
- cluster / persistence / stream の lock surface を同時に移行すること
- lock 実装候補を網羅的に追加すること
- RwLock driver の全 caller を Phase V-A で追従させること

## Capabilities

### Added Capabilities

- `utils-lock-driver-port`
  - `RuntimeMutex` / `RuntimeRwLock` を driver 差し替え可能な port として提供する

### Modified Capabilities

- `dispatcher-trait-provider-abstraction`
  - actor-core hot path の shared wrapper が lock driver factory を選択可能になる
- `mailbox-runnable-drain`
  - `Mailbox` が hot path instrumentation driver で実行可能になる

## Impact

### 影響コード (Phase V-A の中心)

- `modules/utils-core/src/core/sync/`
- `modules/utils-adaptor-std/src/`
- `modules/actor-core/src/core/kernel/actor/actor_ref/actor_ref_sender_shared.rs`
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatcher_sender.rs`
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher_shared.rs`
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared.rs`
- `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs`
- `modules/actor-core/src/core/kernel/actor/actor_cell.rs`

### 波及影響

Phase V-A は hot path 4 型を主対象とするが、型パラメータの伝播により次の transitive caller も同時に触る可能性が高い。

- `DispatcherSender`
- `ActorCell`
- hot path 周辺の unit / integration tests

したがって「実装対象は 4 型だけ」という意味ではなく、「設計判断の主対象が 4 型である」と理解する。

### 設計上の効果

- actor-core 再入 hot path に debug driver を差し込める
- std 環境で `SpinSyncMutex` 以外の driver を選択可能になる
- workspace 全体の全面移行を後続 phase に分離できる

## Success Criteria

- actor-core hot path で `DebugSpinSyncMutex` を選んだ test configuration が構築できる
- std 環境で `StdSyncMutex` を driver 候補として選べる
- 再入 deadlock を再現・観測するための driver 差し替え seam が成立する
- cluster / persistence / stream を未移行のままでも actor-core の導入価値が成立する
