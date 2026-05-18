# persistence-core-kernel Rust 2018 モジュール再編計画

## Summary

`modules/persistence-core-kernel/src` 直下に並んでいる永続化関連型を、Pekko の `persistence`
パッケージ構造を参考に Rust 2018 方式で再配置する。後方互換の root 再エクスポートや旧パス
fallback は残さず、利用側の `use` も新しいモジュールパスへ書き換える。

## Key Changes

- `lib.rs` は root バレルではなく、意味のある公開モジュールだけを公開する。
  - `journal`
  - `snapshot`
  - `state`
  - `persistent`
  - `delivery`
  - `fsm`
  - `extension`
  - `error`
- `journal` 配下へ journal trait / actor / protocol / event adapter / in-memory journal / plugin proxy を移動する。
- `snapshot` 配下へ snapshot model / store / actor / protocol / criteria / in-memory snapshot store を移動する。
- `state` 配下へ durable state store 関連型を移動する。
- `persistent` 配下へ persistent actor / context / repr / recovery / props / stash overflow 関連型を移動する。
- `delivery` 配下へ at-least-once delivery 関連型を移動する。
- `fsm` 配下へ `PersistentFsm` を移動する。
- `extension` 配下へ persistence extension 関連型を移動する。
- `error` 配下へ `PersistenceError` を移動する。

## Implementation Details

- Rust 2018 モジュール方式で作る。
  - `journal.rs` + `journal/*.rs`
  - `snapshot.rs` + `snapshot/*.rs`
  - `state.rs` + `state/*.rs`
  - `persistent.rs` + `persistent/*.rs`
  - `delivery.rs` + `delivery/*.rs`
  - `fsm.rs` + `fsm/*.rs`
  - `extension.rs` + `extension/*.rs`
  - `error.rs` + `error/*.rs`
  - `mod.rs` は作らない。
- 旧 root パスは削除する。
  - `fraktor_persistence_core_kernel_rs::Journal` は使えなくする。
  - 正は `fraktor_persistence_core_kernel_rs::journal::Journal`。
  - `SnapshotStore` は `snapshot::SnapshotStore`。
  - `PersistentActor` は `persistent::PersistentActor`。
  - `PersistenceExtension` は `extension::PersistenceExtension`。
  - `PersistenceError` は `error::PersistenceError`。
- crate 内部と外部利用側の `use` を新パスへ置換する。
- sibling test 配置は維持する。

## Test Plan

- `cargo fmt --check`
- `cargo test -p fraktor-persistence-core-kernel-rs -- --nocapture`
- `cargo test -p fraktor-persistence-core-typed-rs -- --nocapture`
- `cargo test -p fraktor-persistence-core-kernel-rs --tests -- --nocapture`
- 必要なら最終確認で `./scripts/ci-check.sh ai all`

## Assumptions

- 後方互換性は維持しない。
- 旧パスへの fallback / deprecated re-export は作らない。
- 目的は機能追加ではなく、Pekko 互換の概念境界に沿った見通し改善。
- `persistence-core-typed` 固有の対策は kernel の構造には混ぜない。
- `persistence-core-typed` 自体のパッケージ再設計は今回の主対象外で、import 追従だけ行う。
