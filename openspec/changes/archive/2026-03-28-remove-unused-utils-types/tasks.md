## 1. core/sync の未使用型を削除

- [x] 1.1 `core/sync/rc_shared.rs` と tests を削除し、sync.rs からモジュール宣言・re-export を除去
- [x] 1.2 `core/sync/static_ref_shared.rs` と tests を削除し、sync.rs から除去
- [x] 1.3 `core/sync/function/` ディレクトリ全体を削除し、sync.rs から除去
- [x] 1.4 `core/sync/flag.rs` と tests を削除し、sync.rs から除去
- [x] 1.5 `core/sync/state.rs` と tests を削除し、sync.rs から除去
- [x] 1.6 `core/sync/interrupt/` ディレクトリ全体を削除し、sync.rs から除去
- [x] 1.7 `core/sync/async_mutex_like/` ディレクトリ全体を削除し、sync.rs から除去

## 2. std/collections の未使用モジュールを削除

- [x] 2.1 `std/collections/` ディレクトリ全体を削除し、std.rs からモジュール宣言を除去

## 3. clippy.toml の更新

- [x] 3.1 `modules/actor/clippy.toml` と `modules/cluster/clippy.toml` から AsyncMutexLike の参照を除去

## 4. 検証

- [x] 4.1 `cargo check --workspace` が通ることを確認
- [x] 4.2 `cargo test -p fraktor-utils-rs --lib` が通ることを確認
- [x] 4.3 dylint lint が通ることを確認
