## Why

`modules/utils-core/src/core/sync/` の同期プリミティブに不要な中間層が堆積しており、構造が本質的でない。具体的には `SharedLock<T>` が内部で `RuntimeMutexSharedLockBackend → RuntimeMutex → LockDriver` と3段のラッピングを行っているが、`RuntimeMutex` は `LockDriver` への単なる委譲であり、存在理由がない。また Mutex系には型消去ラッパー `SharedLock` があるのに RwLock系には対応する `SharedRwLock` が存在せず、設計の対称性が崩れている。

## What Changes

- **`SharedLock<T>` の内部構造簡素化**: `RuntimeMutexSharedLockBackend` が `RuntimeMutex` を経由せず `LockDriver` を直接保持するよう変更
- **`SharedRwLock<T>` の新設**: `SharedLock<T>` と対称な設計で、`RwLockDriver` の型パラメータ `D` を消去する closure-based API を提供
- **BREAKING** `RuntimeMutex<T, D>` の廃止: 全使用箇所を `SharedLock<T>` または `LockDriver<T>` 直接使用に移行
- **BREAKING** `RuntimeRwLock<T, D>` の廃止: 全使用箇所を `SharedRwLock<T>` に移行
- **`NoStdMutex<T>` 型エイリアスの廃止**: `RuntimeMutex` ベースのため不要に

## Capabilities

### New Capabilities
- `shared-rwlock`: `SharedRwLock<T>` — `RwLockDriver` の型パラメータ消去 + `ArcShared` 内蔵の closure-based 共有ラッパー。`with_read` / `with_write` API を提供

### Modified Capabilities
- `actor-runtime-safety`: `RuntimeMutex`/`RuntimeRwLock` の廃止に伴い、同期プリミティブの選択メカニズムが `SharedLock`/`SharedRwLock` + `ActorLockProvider` に一本化

## Impact

- **対象モジュール**: `utils-core`（新設・修正）、`actor-core`（RuntimeMutex/RuntimeRwLock 全使用箇所の移行）、`actor-adaptor-std`（LockProvider 実装の更新）、`persistence-core`、`cluster-core`、`stream-core`（RuntimeRwLock 使用箇所の移行）
- **影響規模**: `ArcShared<RuntimeMutex<T>>` パターンが多数、`ArcShared<RuntimeRwLock<T>>` パターンが約14箇所。`SharedLock` 経由の `ActorLockProvider` 系は少数。段階的に移行可能
- **API 変更**: guard 返却 API（`lock()` → Guard）から closure API（`with_lock(|v| ...)` / `with_read(|v| ...)` / `with_write(|v| ...)`）への移行。プロジェクトの「ロック区間はメソッド内に閉じる」ポリシーと整合
- **`SharedAccess` trait との統合**: 既存の `SharedAccess<B>` trait（`with_read`/`with_write`）を `SharedLock`/`SharedRwLock` が直接実装することで、`*Shared` 型での手動 `impl SharedAccess` を削減可能
- **二重 Arc 注意点**: `SharedLock`/`SharedRwLock` は `ArcShared` 内蔵のため、既に `ArcShared` で包まれた構造体のフィールドでは二重 Arc になる。メモリオーバーヘッドは軽微なため一貫性を優先して許容する
