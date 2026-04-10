## 1. SharedLock 内部の地層解消

- [ ] 1.1 `shared_lock.rs` 内の `RuntimeMutexSharedLockBackend<T, D>` を `LockDriverBackend<T, D>` にリネームし、内部フィールドを `RuntimeMutex<T, D>` から `D: LockDriver<T>` 直接保持に変更する
- [ ] 1.2 `SharedLock` の固有メソッド `with_read` を廃止する。既存の呼び出し箇所を `with_lock` に移行する
- [ ] 1.3 `SharedLock` に `impl SharedAccess<T> for SharedLock<T>` を追加する（`with_read` / `with_write` ともに `with_lock` に委譲）
- [ ] 1.4 `SharedLock` に `downgrade()` → `WeakSharedLock<T>` を追加する（`shared_lock.rs` 内に同居）
- [ ] 1.5 `SharedLock` の既存テストが全パスすることを確認する
- [ ] 1.6 `./scripts/ci-check.sh ai dylint` を実行し、lint エラーがないことを確認する

## 2. SharedRwLock の新設

- [ ] 2.1 `shared_rw_lock.rs` に `SharedRwLockBackend<T>` trait（private）と `RwLockDriverBackend<T, D>` を定義する（`SharedLock` と同様に同一ファイル内に配置）
- [ ] 2.2 `SharedRwLock<T>` を実装する（`new_with_driver`, `with_read`, `with_write`, `Clone`）
- [ ] 2.3 `SharedRwLock<T>` に `impl SharedAccess<T>` を追加する
- [ ] 2.4 `SharedRwLock<T>` に `downgrade()` → `WeakSharedRwLock<T>` を追加する（`shared_rw_lock.rs` 内に同居）
- [ ] 2.5 `sync.rs` に `SharedRwLock`, `WeakSharedLock`, `WeakSharedRwLock` の `pub use` を追加する
- [ ] 2.6 `SharedRwLock` のユニットテストを作成する（構築、read/write、Clone 共有、SharedAccess 委譲、downgrade/upgrade）
- [ ] 2.7 `./scripts/ci-check.sh ai dylint` を実行し、lint エラーがないことを確認する

## 3. 非推奨マークの付与

CI は `-Dwarnings --force-warn deprecated` のため、deprecated 警告があってもビルドは通る。先に付与することでコンパイラが残存箇所を警告で教えてくれるため、移行漏れ防止に有効。

- [ ] 3.1 `RuntimeMutex` に `#[deprecated(note = "use SharedLock instead")]` を付与する
- [ ] 3.2 `RuntimeRwLock` に `#[deprecated(note = "use SharedRwLock instead")]` を付与する
- [ ] 3.3 `NoStdMutex` 型エイリアスに `#[deprecated(note = "use SharedLock instead")]` を付与する
- [ ] 3.4 `./scripts/ci-check.sh ai dylint` を実行し、lint エラーがないことを確認する

## 4. RuntimeRwLock → SharedRwLock への移行（約14箇所）

deprecated 警告を手がかりに残存箇所を特定しながら移行する。

- [ ] 4.1 `actor-core` の RwLock 系 `*Shared` 型（`EventStreamShared`, `SchedulerShared`, `DeadLetterShared`, `MessageInvokerShared`, `MiddlewareShared`）を `SharedRwLock<T>` に移行する。`.read()` → `.with_read(|v| ...)`, `.write()` → `.with_write(|v| ...)` の closure 化を含む
- [ ] 4.2 `actor-core` の `SystemStateShared` を `SharedRwLock<T>` に、`SystemStateWeak` を `WeakSharedRwLock<T>` に移行する
- [ ] 4.3 `actor-core` の `SerializationRegistry` 内部の `RuntimeRwLock` フィールドを `SharedRwLock<T>` に移行する
- [ ] 4.4 `./scripts/ci-check.sh ai dylint` を実行し、lint エラーがないことを確認する

## 5. RuntimeMutex → SharedLock への移行（多数）

`ArcShared<RuntimeMutex<T>>` パターンと `RuntimeMutex<T>` 直接保持パターンの両方を移行する。guard チェーン `.lock().method()` を `.with_lock(|v| v.method())` に closure 化する書き換えを含む。既存の排他ロックセマンティクスは維持し、Mutex → RwLock への変更は行わない。deprecated 警告を手がかりに残存箇所を特定しながら移行する。

- [ ] 5.1 `actor-core` の `dispatch` 配下を移行する（`SharedMessageQueue`, MessageQueue 各種: `UnboundedDequeMessageQueue`, `BoundedPriorityMessageQueue`, `UnboundedPriorityMessageQueue` 等）
- [ ] 5.2 `actor-core` の `mailbox` 配下を移行する（`MailboxQueueHandles`, `MailboxPollFuture` 等）
- [ ] 5.3 `actor-core` の `pattern` 配下を移行する（`CircuitBreakerShared` 等）
- [ ] 5.4 `actor-core` の `system` 配下を移行する（`CoordinatedShutdown`, `TerminationState`, `CellsShared`, `SerializationExtensionShared` 等）
- [ ] 5.5 `actor-core` の `actor`（kernel 層）配下を移行する（`ActorCell`, `ActorShared`, `ActorContext`, `TickDriverConfig`, `TickDriverHandle`, `DiagnosticsRegistry`, `ActorFactoryShared` 等）
- [ ] 5.6 `actor-core` の `typed` 配下を移行する（`TimerSchedulerShared`, `AdaptMessage`, `AdapterEnvelope`, routing 系, delivery 系 等）
- [ ] 5.7 `persistence-core` を移行する（`PersistenceExtensionShared`, `JournalActorAdapter`, `SnapshotActorAdapter` 等）
- [ ] 5.8 `cluster-core` を移行する（`ClusterExtension`, `ClusterCore`, `PlacementCoordinatorShared`, `GossiperShared`, `BatchingProducer`, `IdentityLookupShared` 等）
- [ ] 5.9 `stream-core` を移行する（`MaterializerSession` 等）
- [ ] 5.10 `utils-core` の `WaitNodeShared` を移行する
- [ ] 5.11 各 `*Shared` 型の手動 `impl SharedAccess` を `SharedLock`/`SharedRwLock` への委譲に簡素化する
- [ ] 5.12 `./scripts/ci-check.sh ai dylint` を実行し、lint エラーがないことを確認する

## 6. 廃止と最終確認

- [ ] 6.1 deprecated 警告がゼロであることを確認する（全使用箇所の移行完了の証明）
- [ ] 6.2 `RuntimeMutex`, `RuntimeRwLock`, `NoStdMutex` の定義ファイルとテストを削除する
- [ ] 6.3 `sync.rs` から `pub use` を除去する
- [ ] 6.4 `./scripts/ci-check.sh ai all` で全 CI を通す
