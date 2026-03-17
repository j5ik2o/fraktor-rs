# actor std wrapper 整理計画

## 目的

`modules/actor/src/std` 配下にある再エクスポート専用の薄い `.rs` ファイルを削減し、`std` の構造を本質的な型中心へ寄せる。

## 方針

1. 公開型を定義しているファイルは維持する
2. モジュール宣言と `pub use` だけを持つ wrapper ファイルは `std.rs` 側へ吸収する
3. 既存の公開モジュールパスは可能な範囲で維持しつつ、不要な wrapper ファイルだけを削除する
4. 削除済み wrapper が復活しないようにテストで固定する

## 対象

- `modules/actor/src/std/actor.rs`
- `modules/actor/src/std/dispatch.rs`
- `modules/actor/src/std/dispatch/dispatcher.rs`
- `modules/actor/src/std/event.rs`
- `modules/actor/src/std/event/logging.rs`
- `modules/actor/src/std/event/stream.rs`
- `modules/actor/src/std/props.rs`
- `modules/actor/src/std/scheduler.rs`
- `modules/actor/src/std/system.rs`
- `modules/actor/src/std/typed.rs`
- `modules/actor/src/std/typed/actor.rs`

## 実施手順

1. `std.rs` に中間モジュール宣言と再エクスポートを集約する
2. 各 wrapper ファイルを削除する
3. `std/tests.rs` に削除対象を追加する
4. `cargo test -p fraktor-actor-rs std::tests` を実行する
5. 最後に `./scripts/ci-check.sh ai all` を実行する