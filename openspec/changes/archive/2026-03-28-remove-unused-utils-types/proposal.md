## Why

`modules/utils` に workspace 内のどのクレートからも使用されていない型・モジュールが約20ファイル残存している。「Less is more」「YAGNI」の原則に従い、未使用コードを削除して保守コストを下げる。

調査の結果、以下が外部クレートおよび utils 内部の本番コードから一切参照されていないことを確認済み:
- `core/sync`: RcShared, StaticRefShared, SharedFactory, SharedFn, AtomicFlag, AtomicState, InterruptPolicy 系, AsyncMutexLike, SpinAsyncMutex
- `std/collections`: MpscBackend（std/collections/ 配下全体）

注意: `std::StdSyncMutex` と `std::StdSyncRwLock` は `RuntimeMutexBackend` / `RuntimeRwLockBackend` として utils 内部で使用されているため削除不可。

## What Changes

- **BREAKING** `core::sync::RcShared` を削除
- **BREAKING** `core::sync::StaticRefShared` を削除
- **BREAKING** `core::sync::function` モジュール（SharedFactory, SharedFn）を削除
- **BREAKING** `core::sync::flag` モジュール（AtomicFlag）を削除
- **BREAKING** `core::sync::state` モジュール（AtomicState）を削除
- **BREAKING** `core::sync::interrupt` モジュール（InterruptPolicy, CriticalSectionInterruptPolicy, NeverInterruptPolicy）を削除
- **BREAKING** `core::sync::async_mutex_like` モジュール（AsyncMutexLike, SpinAsyncMutex）を削除
- **BREAKING** `std::collections` モジュール（MpscBackend）を削除
- 対応するモジュール宣言・re-export を sync.rs, std.rs から除去

## Capabilities

### Modified Capabilities

- `utils-dead-code-removal`: 未使用の型・モジュールを削除し utils の公開面を縮小する

## Impact

- 影響コード: `modules/utils/src/core/sync/` 配下 ~17ファイル、`modules/utils/src/std/collections/` 配下 ~3ファイル
- 影響 API: 削除される型は workspace 内で未使用のため実質的な影響なし
- リスク: 低（未使用であることを grep で確認済み）

## Non-goals

- 使用中の型・モジュールの変更
- std::StdSyncMutex / StdSyncRwLock の削除（RuntimeMutex のバックエンドとして内部使用中）
- utils の設計・構造の変更（削除のみ）
