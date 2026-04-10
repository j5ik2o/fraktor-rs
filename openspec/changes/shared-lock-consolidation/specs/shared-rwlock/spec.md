## ADDED Requirements

### Requirement: SharedRwLock は型パラメータ消去された共有 RwLock を提供する

`SharedRwLock<T>` は `RwLockDriver` の型パラメータ `D` を消去し、`ArcShared` を内蔵した closure-based の共有 RwLock ラッパーとして提供されなければならない (MUST)。`SharedLock<T>` と対称な設計とする。

#### Scenario: SharedRwLock は任意の RwLockDriver で構築できる
- **WHEN** `SharedRwLock::new_with_driver::<D>(value)` を `D: RwLockDriver<T> + Send + Sync + 'static` で呼ぶ
- **THEN** `SharedRwLock<T>` が返される
- **AND** 型パラメータ `D` は `SharedRwLock<T>` の型シグネチャに現れない

#### Scenario: with_read は共有ロック下で読み取りアクセスを提供する
- **WHEN** `shared_rw_lock.with_read(|value| ...)` を呼ぶ
- **THEN** closure に `&T` が渡される
- **AND** closure の戻り値が `with_read` の戻り値として返される
- **AND** 複数スレッドが同時に `with_read` を呼べる（共有ロック）
- **AND** `SharedLock::with_read`（排他ロック下の読み取り）とはセマンティクスが異なることに注意

#### Scenario: with_write は排他ロック下で書き込みアクセスを提供する
- **WHEN** `shared_rw_lock.with_write(|value| ...)` を呼ぶ
- **THEN** closure に `&mut T` が渡される
- **AND** closure の戻り値が `with_write` の戻り値として返される
- **AND** `with_write` は他の `with_read`/`with_write` と排他的である

#### Scenario: SharedRwLock は Clone 可能である
- **WHEN** `SharedRwLock<T>` を clone する
- **THEN** clone は同じ内部データを共有する（`ArcShared` によるリファレンスカウント）

### Requirement: SharedRwLock は WeakSharedRwLock への downgrade を提供する

`SharedRwLock<T>` は `downgrade()` メソッドにより `WeakSharedRwLock<T>` を返さなければならない (MUST)。`WeakSharedRwLock<T>` は `upgrade()` で元の `SharedRwLock<T>` を復元できる。

#### Scenario: downgrade した参照は upgrade で復元できる
- **WHEN** `shared_rw_lock.downgrade()` で `WeakSharedRwLock<T>` を取得する
- **AND** 元の `SharedRwLock<T>` がまだ生存している
- **THEN** `weak.upgrade()` は `Some(SharedRwLock<T>)` を返す

#### Scenario: 全 strong 参照が drop された後は upgrade が None を返す
- **WHEN** `SharedRwLock<T>` の全 strong 参照が drop された後に `weak.upgrade()` を呼ぶ
- **THEN** `None` が返される

#### Scenario: WeakSharedRwLock は Clone 可能である
- **WHEN** `WeakSharedRwLock<T>` を clone する
- **THEN** clone は同じ weak 参照を共有する

### Requirement: SharedLock は WeakSharedLock への downgrade を提供する

`SharedLock<T>` は `downgrade()` メソッドにより `WeakSharedLock<T>` を返さなければならない (MUST)。対称性のため `SharedRwLock` と同等の API を提供する。

#### Scenario: downgrade した参照は upgrade で復元できる
- **WHEN** `shared_lock.downgrade()` で `WeakSharedLock<T>` を取得する
- **AND** 元の `SharedLock<T>` がまだ生存している
- **THEN** `weak.upgrade()` は `Some(SharedLock<T>)` を返す

#### Scenario: 全 strong 参照が drop された後は upgrade が None を返す
- **WHEN** `SharedLock<T>` の全 strong 参照が drop された後に `weak.upgrade()` を呼ぶ
- **THEN** `None` が返される

#### Scenario: WeakSharedLock は Clone 可能である
- **WHEN** `WeakSharedLock<T>` を clone する
- **THEN** clone は同じ weak 参照を共有する

### Requirement: SharedRwLock は SharedAccess を実装する

`SharedRwLock<T>` は既存の `SharedAccess<T>` trait を実装しなければならない (MUST)。

#### Scenario: SharedRwLock は SharedAccess の with_read を委譲する
- **WHEN** `SharedAccess::with_read` を `SharedRwLock<T>` に対して呼ぶ
- **THEN** 内部の共有ロックを取得し、closure に `&T` を渡す
- **AND** `SharedRwLock::with_read` と同一の動作をする

#### Scenario: SharedRwLock は SharedAccess の with_write を委譲する
- **WHEN** `SharedAccess::with_write` を `SharedRwLock<T>` に対して呼ぶ
- **THEN** 内部の排他ロックを取得し、closure に `&mut T` を渡す
- **AND** `SharedRwLock::with_write` と同一の動作をする

### Requirement: SharedLock は SharedAccess を実装する

`SharedLock<T>` は既存の `SharedAccess<T>` trait を実装しなければならない (MUST)。

#### Scenario: SharedLock は SharedAccess の with_read / with_write を排他ロックで委譲する
- **WHEN** `SharedAccess::with_read` または `SharedAccess::with_write` を `SharedLock<T>` に対して呼ぶ
- **THEN** 内部の排他ロック（Mutex）を取得し、closure を実行する
- **AND** `SharedLock::with_lock` と同一のロックセマンティクスを持つ

### Requirement: 移行時にロックセマンティクスを変更してはならない

`RuntimeMutex` → `SharedLock` / `RuntimeRwLock` → `SharedRwLock` の移行において、ロックのセマンティクス（排他 / 共有読み取り）を変更してはならない (MUST NOT)。

#### Scenario: 既存の RuntimeMutex 使用箇所は SharedLock（排他ロック）に移行される
- **WHEN** 既存コードが `RuntimeMutex<T>` を使用している箇所を移行する
- **THEN** 移行先は `SharedLock<T>` である（排他ロック）
- **AND** `SharedRwLock<T>` への変更（共有読み取りへのセマンティクス変更）は行わない
- **AND** セマンティクス変更が必要な場合は本変更のスコープ外として別 change で提案する

#### Scenario: 既存の RuntimeRwLock 使用箇所は SharedRwLock（共有読み取り）に移行される
- **WHEN** 既存コードが `RuntimeRwLock<T>` を使用している箇所を移行する
- **THEN** 移行先は `SharedRwLock<T>` である（共有読み取り + 排他書き込み）
- **AND** `SharedLock<T>` への変更（排他ロックへのセマンティクス変更）は行わない

### Requirement: SharedLock の内部構造は LockDriver を直接保持する

`SharedLock<T>` の内部実装は `RuntimeMutex` を経由せず、`LockDriver<T>` を直接保持しなければならない (MUST)。

#### Scenario: SharedLockBackend の実装は LockDriver を直接使用する
- **WHEN** `SharedLock::new_with_driver::<D>(value)` で内部バックエンドを構築する
- **THEN** バックエンドは `D: LockDriver<T>` を直接フィールドとして保持する
- **AND** `RuntimeMutex<T, D>` を経由しない

#### Scenario: SharedLock の外部 API は変更されない
- **WHEN** 既存コードが `SharedLock::new_with_driver`, `with_lock`, `with_read` を呼ぶ
- **THEN** 動作は従来と同一である

### Requirement: 全 crate の RuntimeMutex / RuntimeRwLock 使用箇所は同一セマンティクスで移行される

`actor-core` 以外の crate（`persistence-core`, `cluster-core`, `stream-core`, `utils-core`）における `RuntimeMutex` / `RuntimeRwLock` の使用箇所も、同一の移行ルールで `SharedLock` / `SharedRwLock` に移行されなければならない (MUST)。

#### Scenario: persistence-core の RuntimeMutex 使用箇所は SharedLock に移行される
- **WHEN** `persistence-core` 内の `RuntimeMutex` 使用箇所（`PersistenceExtensionShared`, `JournalActorAdapter`, `SnapshotActorAdapter` 等）を確認する
- **THEN** すべて `SharedLock<T>` に移行されている
- **AND** 既存の排他ロックセマンティクスが維持されている（Mutex → RwLock への変更なし）

#### Scenario: cluster-core の RuntimeMutex 使用箇所は SharedLock に移行される
- **WHEN** `cluster-core` 内の `RuntimeMutex` 使用箇所（`ClusterExtension`, `ClusterCore`, `GossiperShared`, `BatchingProducer` 等）を確認する
- **THEN** すべて `SharedLock<T>` に移行されている
- **AND** 既存の排他ロックセマンティクスが維持されている

#### Scenario: stream-core の RuntimeMutex 使用箇所は SharedLock に移行される
- **WHEN** `stream-core` 内の `RuntimeMutex` 使用箇所（`MaterializerSession` 等）を確認する
- **THEN** すべて `SharedLock<T>` に移行されている
- **AND** 既存の排他ロックセマンティクスが維持されている

#### Scenario: utils-core の RuntimeMutex 使用箇所は SharedLock に移行される
- **WHEN** `utils-core` 内の `RuntimeMutex` 使用箇所（`WaitNodeShared` 等）を確認する
- **THEN** すべて `SharedLock<T>` に移行されている
- **AND** 既存の排他ロックセマンティクスが維持されている

### Requirement: RuntimeMutex と RuntimeRwLock は廃止される

`RuntimeMutex<T, D>` と `RuntimeRwLock<T, D>` は全使用箇所を `SharedLock<T>` / `SharedRwLock<T>` に移行した後、廃止されなければならない (MUST)。

#### Scenario: RuntimeMutex の使用箇所はゼロになる
- **WHEN** 移行完了後にコードベースを検索する
- **THEN** `RuntimeMutex` への参照は定義ファイルとテスト以外に存在しない
- **AND** `RuntimeMutex` の型定義は削除される

#### Scenario: RuntimeRwLock の使用箇所はゼロになる
- **WHEN** 移行完了後にコードベースを検索する
- **THEN** `RuntimeRwLock` への参照は定義ファイルとテスト以外に存在しない
- **AND** `RuntimeRwLock` の型定義は削除される

#### Scenario: NoStdMutex 型エイリアスは廃止される
- **WHEN** 移行完了後にコードベースを検索する
- **THEN** `NoStdMutex` への参照は存在しない
- **AND** `NoStdMutex` の型エイリアス定義は削除される
