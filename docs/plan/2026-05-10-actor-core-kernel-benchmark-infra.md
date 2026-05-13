# actor-core-kernel ベンチマーク基盤整備計画

## Summary

`actor-core-kernel` に Criterion ベースの mailbox/queue マイクロベンチを追加する。初期スコープは actor 実行器込みではなく、`actor-core-kernel` 単体で測れる mailbox 中心に限定する。CI/通常チェックでは実測せず、`cargo check --benches` によるビルド保証までにする。

## Key Changes

- `modules/actor-core-kernel/Cargo.toml` に `[[bench]] name = "mailbox"` を追加し、`harness = false` を指定する。既存の `criterion` / `critical-section/std` dev-dependencies を使い、新規依存は追加しない。
- `modules/actor-core-kernel/benches/mailbox.rs` を新設し、以下の Criterion group を作る。
  - `message_queue_enqueue`: `UnboundedMessageQueue`、`BoundedMessageQueue`、`UnboundedControlAwareMessageQueue`、priority queue の batch enqueue。
  - `message_queue_drain`: 事前投入した batch を `MessageQueue::dequeue` で drain。
  - `mailbox_enqueue`: `Mailbox::new(MailboxPolicy::unbounded(None))` と bounded policy の `enqueue_user` wrapper overhead。
  - `mailbox_overflow`: bounded capacity 超過時の `DropNewest` / `DropOldest` 経路。
- batch size は初期値 `[1, 64, 1024]`、Criterion のローカル実測デフォルトは通常設定、smoke 実行は `--warm-up-time 0.1 --measurement-time 0.2 --sample-size 10` を使う。
- private な `LockFreeMpscQueue` は公開しない。ベンチは公開済みの `MessageQueue` 実装と `Mailbox` API だけを使う。
- `Makefile.toml` の壊れている `actor-core-perf` を、存在しない `perf_mailbox` テストと存在しない `std` feature から切り離す。
  - `actor-core-perf`: `cargo check -p fraktor-actor-core-kernel-rs --benches`
  - `actor-core-bench`: `cargo bench -p fraktor-actor-core-kernel-rs --bench mailbox`
- `scripts/ci-check.sh` の default `all` には実測を追加しない。`perf|bench|performance` 経路に actor-core-kernel の `cargo check --benches` だけを追加する。

## Interfaces

- 公開 Rust API の追加・変更はしない。
- 新しい実行インターフェース:
  - ビルド確認: `cargo check -p fraktor-actor-core-kernel-rs --benches`
  - 実測: `cargo bench -p fraktor-actor-core-kernel-rs --bench mailbox`
  - 軽量 smoke 実測: `cargo bench -p fraktor-actor-core-kernel-rs --bench mailbox -- --warm-up-time 0.1 --measurement-time 0.2 --sample-size 10`

## Test Plan

- `cargo check -p fraktor-actor-core-kernel-rs --benches`
- `cargo bench -p fraktor-actor-core-kernel-rs --bench mailbox -- --warm-up-time 0.1 --measurement-time 0.2 --sample-size 10`
- `cargo clippy -p fraktor-actor-core-kernel-rs --benches -- -D warnings`
- 既存 mailbox 回帰確認として `cargo test -p fraktor-actor-core-kernel-rs dispatch::mailbox`

## Assumptions

- 初期スコープは mailbox/queue の基盤測定に限定し、spawn/tell/ping-pong は既存の `actor-adaptor-std` bench 側に残す。
- ベンチ結果の閾値判定は導入しない。性能回帰検出はまず人間が Criterion レポートを比較する運用にする。
- `actor-core-kernel` の no_std production 境界は変えない。bench target だけが std 環境で動く。
