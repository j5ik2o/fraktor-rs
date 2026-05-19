# utils-dead-code-removal Specification

## Purpose
`modules/utils` の公開 API から未使用の共有・同期補助型を排除し、workspace で実際に使われている最小構成だけを維持する。

## Requirements
### Requirement: 未使用の共有・同期補助型は公開 API に存在しない
`modules/utils` は、workspace 内で未使用になった共有・同期補助型を公開 API として提供してはならない（MUST NOT）。`RcShared`, `StaticRefShared`, `SharedFactory`, `SharedFn`, `AtomicFlag`, `AtomicState`, `InterruptPolicy`, `CriticalSectionInterruptPolicy`, `NeverInterruptPolicy`, `AsyncMutexLike`, `SpinAsyncMutex`, `MpscBackend`, `SyncMpscQueueShared`, `SyncSpscQueueShared`, `SyncPriorityQueueShared`, `SyncMpscProducerShared`, `SyncMpscConsumerShared`, `SyncSpscProducerShared`, `SyncSpscConsumerShared`, `StdSyncMutex`, `StdSyncMutexGuard`, `StdSyncRwLock`, `StdSyncRwLockReadGuard`, `StdSyncRwLockWriteGuard`, `StdMutex`, `RuntimeMutexBackend`, `RuntimeRwLockBackend`, `SyncMutexLike`, `SyncRwLockLike` は公開 API から除外されていなければならない（MUST）。

`SyncQueueShared` および `SyncFifoQueueShared` は production 利用 (actor-core mailbox の `UserQueueShared` と stream-core の `StreamBuffer.queue`) があるため禁止リストには含めない。ただし `SyncQueueShared` は型パラメータ `M` (mutex backend) を持たず、内部で `SpinSyncMutex` を直接使うよう monomorphize されていなければならない（MUST）。

#### Scenario: 未使用型を import できない
- **WHEN** workspace の任意の crate から `modules/utils` の公開 API を通じて未使用型を import しようとする
- **THEN** その型は解決できず、コンパイル時に参照できない

#### Scenario: 残る型は workspace で使われるものだけ
- **WHEN** `modules/utils` の公開 API 一覧を確認する
- **THEN** workspace で参照されている共有・同期補助型だけが残っている

#### Scenario: SyncQueueShared は SpinSyncMutex に monomorphize されている
- **WHEN** `SyncQueueShared<T, K, B>` の公開 API を確認する
- **THEN** 第 4 型パラメータ `M` は存在せず、内部の `inner` フィールドは `ArcShared<SpinSyncMutex<SyncQueue<T, K, B>>>` 型として固定されている
- **AND** `SyncMutexLike` trait の generic bound としての caller は存在しない

#### Scenario: SyncFifoQueueShared alias も 2 パラメータ化されている
- **WHEN** `SyncFifoQueueShared<T, B>` の公開 API を確認する
- **THEN** 型 alias は `SyncQueueShared<T, FifoKey, B>` として 2 パラメータで定義されており、第 3 型パラメータ `M` は存在しない

#### Scenario: SpinSyncRwLock は inherent な read/write メソッドを持つ
- **WHEN** `SpinSyncRwLock<T>` の公開 API を確認する
- **THEN** trait import なしで `lock.read()` / `lock.write()` が呼べる
- **AND** これらのメソッドは `SyncRwLockLike` trait method ではなく inherent method として提供されている

#### Scenario: 同期プリミティブの alias 名は維持される
- **WHEN** workspace の crate が `RuntimeMutex<T>` / `RuntimeRwLock<T>` / `NoStdMutex<T>` を import する
- **THEN** これらの alias は引き続き解決でき、`SpinSyncMutex<T>` / `SpinSyncRwLock<T>` の直接 alias として機能する
- **AND** 既存 caller (合計 173 ファイル) は無修正で動作する
