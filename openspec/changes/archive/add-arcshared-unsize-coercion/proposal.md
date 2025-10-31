## Why
`ArcShared` はデフォルトで `alloc::sync::Arc` を内部で使用しつつ、feature `force-portable-arc` 有効時には portable-atomic ベースの `Arc` に切り替えられるが、現状では `ArcShared<T>` から `ArcShared<dyn Trait>` への自動型強制が機能せず、利用者は `.into_dyn()` などの明示的変換に依存している。Rust nightly が提供する `CoerceUnsized` と `DispatchFromDyn` を活用すれば、この操作を自動的に行える。

## What Changes
- nightly 専用 feature (`unsize`) の配下で `#![feature(unsize, coerce_unsized)]` を有効化する。
- `ArcShared` に対して `CoerceUnsized`/`DispatchFromDyn` 実装を追加し、`ArcShared<T>` から `ArcShared<dyn Trait>` への自動 coercion を許可する。
- 既存の実装方針に従い、`force-portable-arc` feature が有効な場合は `portable_atomic_util::Arc`、そうでなければ `alloc::sync::Arc` を内部表現として用いる。
- `#[cfg(feature = "unsize")]` で nightly 専用コードを明示する。

## Impact
- nightly + `unsize` feature を有効にした構成でトレイトオブジェクトへの coercion が自動化され、API の ergonomics が向上する。
- stable ビルドでは feature gate により従来通りの挙動が維持される。
- 選択された Arc バックエンド（`force-portable-arc` 時は portable atomic、未指定時は `alloc::sync::Arc`）とのレイアウト互換性を保ちつつ、既存の `Clone`/`Deref` などの実装は維持される。
