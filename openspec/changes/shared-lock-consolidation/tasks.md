## 1. SharedLock 内部の地層解消

- [ ] 1.1 `shared_lock.rs` 内の `RuntimeMutexSharedLockBackend<T, D>` を `LockDriverBackend<T, D>` にリネームし、内部フィールドを `RuntimeMutex<T, D>` から `D: LockDriver<T>` 直接保持に変更する
- [ ] 1.2 `SharedLock` に `impl SharedAccess<T> for SharedLock<T>` を追加する（`with_read` → `with_lock` 委譲、`with_write` → `with_lock` 委譲）
- [ ] 1.3 `SharedLock` に `downgrade()` → `WeakSharedLock<T>` を追加する（`shared_lock.rs` 内に同居）
- [ ] 1.4 `SharedLock` の既存テストが全パスすることを確認する

## 2. SharedRwLock の新設

- [ ] 2.1 `shared_rw_lock.rs` に `SharedRwLockBackend<T>` trait（private）と `RwLockDriverBackend<T, D>` を定義する（`SharedLock` と同様に同一ファイル内に配置）
- [ ] 2.2 `SharedRwLock<T>` を実装する（`new_with_driver`, `with_read`, `with_write`, `Clone`）
- [ ] 2.3 `SharedRwLock<T>` に `impl SharedAccess<T>` を追加する
- [ ] 2.4 `SharedRwLock<T>` に `downgrade()` → `WeakSharedRwLock<T>` を追加する（`shared_rw_lock.rs` 内に同居）
- [ ] 2.5 `sync.rs` に `SharedRwLock`, `WeakSharedLock`, `WeakSharedRwLock` の `pub use` を追加する
- [ ] 2.6 `SharedRwLock` のユニットテストを作成する（構築、read/write、Clone 共有、SharedAccess 委譲、downgrade/upgrade）

## 3. RuntimeRwLock → SharedRwLock への移行（約14箇所）

- [ ] 3.1 `actor-core` の RwLock 系 `*Shared` 型（`EventStreamShared`, `SchedulerShared`, `DeadLetterShared`, `MessageInvokerShared`, `MiddlewareShared`）を `SharedRwLock<T>` に移行する。`.read()` → `.with_read(|v| ...)`, `.write()` → `.with_write(|v| ...)` の closure 化を含む
- [ ] 3.2 `actor-core` の `SystemStateShared` を `SharedRwLock<T>` に、`SystemStateWeak` を `WeakSharedRwLock<T>` に移行する
- [ ] 3.3 `actor-core` の `SerializationRegistry` 内部の `RuntimeRwLock` フィールドを `SharedRwLock<T>` に移行する
- [ ] 3.4 CI を通す

## 4. RuntimeMutex → SharedLock への移行（多数）

`ArcShared<RuntimeMutex<T>>` パターンと `RuntimeMutex<T>` 直接保持パターンの両方を移行する。guard チェーン `.lock().method()` を `.with_lock(|v| v.method())` に closure 化する書き換えを含む。既存の排他ロックセマンティクスは維持し、Mutex → RwLock への変更は行わない。

- [ ] 4.1 `actor-core` の `dispatch` 配下を移行する（`SharedMessageQueue`, MessageQueue 各種: `UnboundedDequeMessageQueue`, `BoundedPriorityMessageQueue`, `UnboundedPriorityMessageQueue` 等）
- [ ] 4.2 `actor-core` の `mailbox` 配下を移行する（`MailboxQueueHandles`, `MailboxPollFuture` 等）
- [ ] 4.3 `actor-core` の `pattern` 配下を移行する（`CircuitBreakerShared` 等）
- [ ] 4.4 `actor-core` の `system` 配下を移行する（`CoordinatedShutdown`, `TerminationState`, `CellsShared`, `SerializationExtensionShared` 等）
- [ ] 4.5 `actor-core` の `actor`（kernel 層）配下を移行する（`ActorCell`, `ActorShared`, `ActorContext`, `TickDriverConfig`, `TickDriverHandle`, `DiagnosticsRegistry`, `ActorFactoryShared` 等）
- [ ] 4.6 `actor-core` の `typed` 配下を移行する（`TimerSchedulerShared`, `AdaptMessage`, `AdapterEnvelope`, routing 系, delivery 系 等）
- [ ] 4.7 `persistence-core` を移行する（`PersistenceExtensionShared`, `JournalActorAdapter`, `SnapshotActorAdapter` 等）
- [ ] 4.8 `cluster-core` を移行する（`ClusterExtension`, `ClusterCore`, `PlacementCoordinatorShared`, `GossiperShared`, `BatchingProducer`, `IdentityLookupShared` 等）
- [ ] 4.9 `stream-core` を移行する（`MaterializerSession` 等）
- [ ] 4.10 `utils-core` の `WaitNodeShared` を移行する
- [ ] 4.11 各 `*Shared` 型の手動 `impl SharedAccess` を `SharedLock`/`SharedRwLock` への委譲に簡素化する
- [ ] 4.12 CI を通す

## 5. 非推奨マークの付与（Phase 3-4 完了後）

- [ ] 5.1 全使用箇所の移行が完了していることを `grep` で確認する
- [ ] 5.2 `RuntimeMutex` に `#[deprecated(note = "use SharedLock instead")]` を付与する
- [ ] 5.3 `RuntimeRwLock` に `#[deprecated(note = "use SharedRwLock instead")]` を付与する
- [ ] 5.4 `NoStdMutex` 型エイリアスに `#[deprecated(note = "use SharedLock instead")]` を付与する
- [ ] 5.5 CI を通す（残存参照がないため deprecated 警告は発生しない）

## 6. 廃止と最終確認

- [ ] 6.1 `RuntimeMutex`, `RuntimeRwLock`, `NoStdMutex` の定義ファイルとテストを削除する
- [ ] 6.2 `sync.rs` から `pub use` を除去する
- [ ] 6.3 `./scripts/ci-check.sh ai all` で全 CI を通す
