# stream crate 境界整理計画

## Summary

`stream-core` を `stream-core-kernel` に改名し、typed actor 依存を持つ Pub/Sub 連携だけを新 crate `stream-core-actor-typed` へ移す。最終的な依存方向は次に固定する。

```text
fraktor-stream-core-kernel-rs
  -> fraktor-actor-core-kernel-rs

fraktor-stream-core-actor-typed-rs
  -> fraktor-stream-core-kernel-rs
  -> fraktor-actor-core-typed-rs

fraktor-stream-adaptor-std-rs
  -> fraktor-stream-core-kernel-rs
```

## Key Changes

- `modules/stream-core` を `modules/stream-core-kernel` に改名し、package 名を `fraktor-stream-core-kernel-rs`、crate import 名を `fraktor_stream_core_kernel_rs` に変更する。
- root `Cargo.toml` の workspace member / workspace dependency、および downstream の `Cargo.toml` と `use fraktor_stream_core_rs::*` を新 crate 名へ更新する。
- `stream-core-kernel` から `fraktor-actor-core-typed-rs` dependency を削除する。`ActorSource` / `ActorSink` は typed actor crate に依存していないため kernel 側に残す。
- 新規 `modules/stream-core-actor-typed` を追加し、package 名を `fraktor-stream-core-actor-typed-rs`、crate import 名を `fraktor_stream_core_actor_typed_rs` にする。
- 旧 `TopicPubSub` 実装とテストを新 crate へ移し、公開名は Pekko に合わせて `PubSub` に変更する。`TopicPubSub` alias / compatibility re-export は残さない。
- `stream-adaptor-std` と showcases は `fraktor-stream-core-kernel-rs` へ更新し、Pub/Sub 利用がある場合だけ `fraktor-stream-core-actor-typed-rs` を追加依存にする。
- `Cargo.lock` を workspace 変更に合わせて更新する。現在有効なドキュメントは新名称へ更新するが、過去の `docs/plan/*` 履歴文書は新規計画ファイル以外は原則書き換えない。

## Public API

- 旧: `fraktor_stream_core_rs::{...}`
- 新: `fraktor_stream_core_kernel_rs::{...}`
- 旧: `fraktor_stream_core_rs::dsl::TopicPubSub`
- 新: `fraktor_stream_core_actor_typed_rs::dsl::PubSub`

`PubSub::source` / `PubSub::sink` の振る舞いは現行 `TopicPubSub` と同等に保つ。型名と crate 境界だけを変更する。

## Test Plan

- `cargo check -p fraktor-stream-core-kernel-rs --no-default-features`
- `cargo test -p fraktor-stream-core-kernel-rs`
- `cargo test -p fraktor-stream-core-actor-typed-rs`
- `cargo test -p fraktor-stream-adaptor-std-rs`
- `cargo test -p fraktor-showcases-std`
- `cargo check --workspace`
- `rg "fraktor_stream_core_rs|fraktor-stream-core-rs|TopicPubSub" Cargo.toml modules showcases tests` で旧 API の残存を確認する。
- `rg "fraktor_actor_core_typed_rs|fraktor-actor-core-typed-rs" modules/stream-core-kernel` で kernel 側に typed 依存が残っていないことを確認する。

## Assumptions

- 破壊的変更として扱い、旧 crate 名・旧型名の互換 alias は提供しない。
- `kernel` は「低レベル実装だけ」ではなく、「no_std な stream 中核 API + runtime substrate」を意味する。
- 今回の責務分離対象は typed actor crate に依存する Pub/Sub 連携のみで、stream DSL 全体の再設計や機能追加は行わない。
