# fraktor-rs

[![ci](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/fraktor-rs.svg)](https://crates.io/crates/fraktor-rs)
[![docs.rs](https://docs.rs/fraktor-rs/badge.svg)](https://docs.rs/fraktor-rs)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/j5ik2o/fraktor-rs)
[![Renovate](https://img.shields.io/badge/renovate-enabled-brightgreen.svg)](https://renovatebot.com)
[![dependency status](https://deps.rs/repo/github/j5ik2o/fraktor-rs/status.svg)](https://deps.rs/repo/github/j5ik2o/fraktor-rs)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![License](https://img.shields.io/badge/License-APACHE2.0-blue.svg)](https://opensource.org/licenses/apache-2-0)
[![Lines of Code](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/j5ik2o/fraktor-rs/refs/heads/main/.github/badges/tokei_badge.json)](https://github.com/j5ik2o/fraktor-rs)

[English](README.md)

fraktor-rs は、Apache Pekko と Proto.Actor に着想を得た、仕様駆動の Rust アクターランタイムです。
ランタイムは `no_std` の core クレートと `std` の adaptor クレートに分けて開発されており、可搬性の高い状態機械と契約を、Tokio、ネットワーク、ホストランタイムの binding から分離しています。

ルートの `fraktor-rs` クレートは、現時点ではプロジェクトメタデータの公開とパッケージ名の予約を担っています。
ランタイム API は [`modules/`](modules) 配下の workspace クレートにあり、動作を確認する最短の入口は実行可能な [`fraktor-showcases-std`](showcases/std) の example 群です。

## 特徴

- actor kernel、typed actor、persistence、remote、cluster、stream、共通 utilities を `no_std` core クレートとして提供します
- `std` adaptor クレートが、Tokio executor、TCP transport、std lock、materializer、cluster delivery helper などのホスト依存部分を分離します
- actor system、supervision、death watch、routing、dispatcher、mailbox、event stream、serialization、remoting、clustering、persistence、stream processing など、Pekko / Proto.Actor 由来の意味論を取り込んでいます
- std showcase で、legacy typed flow、Pekko classic/kernel example、typed example、stream example、advanced remote/persistence scenario を実行できます
- OpenSpec artifact、リポジトリルール、カスタム dylint、CI スクリプトにより、設計意図、モジュール境界、実装チェックをそろえています

## クイックスタート

### 必要なもの

- `rustup`
- Rust toolchain `nightly-2025-12-01` ([`rust-toolchain.toml`](rust-toolchain.toml) で固定)
- フルローカルチェックを実行する場合は `cargo-dylint`、`rustc-dev`、`llvm-tools-preview`

### インストール

```bash
git clone git@github.com:j5ik2o/fraktor-rs.git
cd fraktor-rs
rustup toolchain install nightly-2025-12-01 --component rustfmt --component clippy
```

dylint を含むフル検証を行う場合:

```bash
rustup component add rustc-dev llvm-tools-preview --toolchain nightly-2025-12-01
cargo install cargo-dylint dylint-link
```

### 実行

```bash
cargo run -p fraktor-showcases-std --example getting_started
```

その他の example:

```bash
cargo run -p fraktor-showcases-std --example typed_first_example
cargo run -p fraktor-showcases-std --example stream_first_example
cargo run -p fraktor-showcases-std --features advanced --example remote_lifecycle
cargo run -p fraktor-showcases-std --features advanced --example typed_persistence_effector
```

### 検証

```bash
cargo test -p fraktor-rs
./scripts/ci-check.sh ai all
```

## 使い方

ルート facade が統合ランタイムモジュールを公開するまでは、必要な API 領域を持つクレートを直接使います。

| 領域 | クレート |
| --- | --- |
| Utilities | [`fraktor-utils-core-rs`](modules/utils-core), [`fraktor-utils-adaptor-std-rs`](modules/utils-adaptor-std) |
| Actor runtime | [`fraktor-actor-core-kernel-rs`](modules/actor-core-kernel), [`fraktor-actor-core-typed-rs`](modules/actor-core-typed), [`fraktor-actor-adaptor-std-rs`](modules/actor-adaptor-std) |
| Persistence | [`fraktor-persistence-core-kernel-rs`](modules/persistence-core-kernel), [`fraktor-persistence-core-typed-rs`](modules/persistence-core-typed) |
| Remote | [`fraktor-remote-core-rs`](modules/remote-core), [`fraktor-remote-adaptor-std-rs`](modules/remote-adaptor-std) |
| Cluster | [`fraktor-cluster-core-rs`](modules/cluster-core), [`fraktor-cluster-adaptor-std-rs`](modules/cluster-adaptor-std) |
| Streams | [`fraktor-stream-core-kernel-rs`](modules/stream-core-kernel), [`fraktor-stream-core-actor-typed-rs`](modules/stream-core-actor-typed), [`fraktor-stream-adaptor-std-rs`](modules/stream-adaptor-std) |

showcase クレートは、実行可能な flow の現時点の利用インデックスです。

```bash
cargo run -p fraktor-showcases-std --example request_reply
cargo run -p fraktor-showcases-std --example kernel_supervision
cargo run -p fraktor-showcases-std --example typed_actor_lifecycle
cargo run -p fraktor-showcases-std --example stream_graphs
```

typed persistence effector は、通常の typed `Behavior` を維持したまま hidden child store actor に永続化を委譲します。最小形は次のように `PersistenceEffector::props` で aggregate actor を作ります。

```rust
let props = PersistenceEffector::props(config, |state, effector| {
  Ok(account_behavior(state, effector))
});
```

example の一覧と必要な feature は [`showcases/std/README.md`](showcases/std/README.md) を参照してください。

## ワークスペース構成

| パス | 役割 |
| --- | --- |
| [`src/`](src) | ルート `fraktor-rs` クレートの placeholder とパッケージメタデータ |
| [`modules/utils-core`](modules/utils-core) | 可搬性の高い collection、sync primitive、time helper、atomic、network parsing |
| [`modules/utils-adaptor-std`](modules/utils-adaptor-std) | 標準ライブラリ向け utility adaptor |
| [`modules/actor-core-kernel`](modules/actor-core-kernel) | `no_std` の untyped actor kernel: actor ref、system、dispatch、routing、serialization、pattern、lifecycle |
| [`modules/actor-core-typed`](modules/actor-core-typed) | `no_std` の typed actor facade、DSL、receptionist、pub-sub、delivery、typed event stream、typed system API |
| [`modules/actor-adaptor-std`](modules/actor-adaptor-std) | Std/Tokio actor binding、executor、tick driver、time、event、pattern、test-support helper |
| [`modules/persistence-core-kernel`](modules/persistence-core-kernel) | Event sourcing、journal、snapshot、persistent actor、persistent FSM、durable state、persistence extension |
| [`modules/persistence-core-typed`](modules/persistence-core-typed) | typed actor 向け persistence effector API、snapshot criteria、retention criteria |
| [`modules/remote-core`](modules/remote-core) | `no_std` の remote address、association、envelope、provider、transport port、watcher、wire、failure-detector state machine |
| [`modules/remote-adaptor-std`](modules/remote-adaptor-std) | Std remote extension installer、provider、Tokio TCP transport、I/O worker |
| [`modules/cluster-core`](modules/cluster-core) | Cluster membership、identity、placement、pub-sub、grain、failure detection、topology、metrics、routing |
| [`modules/cluster-adaptor-std`](modules/cluster-adaptor-std) | Std cluster API、local provider wrapping、Tokio gossip transport、pub-sub delivery、optional AWS ECS provider |
| [`modules/stream-core-kernel`](modules/stream-core-kernel) | `no_std` の stream DSL、stage、materialization contract、graph shape、stream ref、queue、kill switch、supervision |
| [`modules/stream-core-actor-typed`](modules/stream-core-actor-typed) | stream DSL 向け typed actor integration |
| [`modules/stream-adaptor-std`](modules/stream-adaptor-std) | Std stream I/O と materializer adaptor |
| [`showcases/std`](showcases/std) | ホスト環境向けの実行可能 example |
| [`tests/e2e`](tests/e2e) | クロスクレート end-to-end test |
| [`lints/`](lints) | プロジェクト構造と Rust 規約のためのカスタム dylint ルール |
| [`openspec/`](openspec) | 仕様駆動設計 artifact、active change、accepted spec |

## ドキュメント

- API ドキュメント: [docs.rs/fraktor-rs](https://docs.rs/fraktor-rs)
- Showcase index: [`showcases/std/README.md`](showcases/std/README.md)
- リポジトリルール: [AGENTS.md](AGENTS.md), [`.agents/rules/project.md`](.agents/rules/project.md)
- OpenSpec 設定: [`openspec/config.yaml`](openspec/config.yaml)
- Lock-free design note: [`docs/guides/lock_free_design.md`](docs/guides/lock_free_design.md)
- 現在の gap report:
  - [Actor](docs/gap-analysis/actor-gap-analysis.md)
  - [Actor mailbox](docs/gap-analysis/actor-mailbox-gap-analysis.md)
  - [Remote](docs/gap-analysis/remote-gap-analysis.md)
  - [Cluster](docs/gap-analysis/cluster-gap-analysis.md)
  - [Persistence](docs/gap-analysis/persistence-gap-analysis.md)
  - [Stream](docs/gap-analysis/stream-gap-analysis.md)
- 参照実装:
  - [`references/pekko`](references/pekko)
  - [`references/protoactor-go`](references/protoactor-go)

## サポート

- Issues: [GitHub Issues](https://github.com/j5ik2o/fraktor-rs/issues)
- リポジトリ知識ベース: [DeepWiki](https://deepwiki.com/j5ik2o/fraktor-rs)

## コントリビューション

- コード変更前に [AGENTS.md](AGENTS.md) と [`.agents/rules/`](.agents/rules) 配下の scoped rule を読んでください
- 振る舞いに影響する変更では OpenSpec を使い、OpenSpec コマンドは `mise exec -- openspec ...` 経由で実行してください
- `*-core` クレートは `no_std` を維持し、ホスト依存の runtime、network、time、Tokio 関連は `*-adaptor-std` クレートに配置してください
- 実行可能 example は `modules/**/examples` ではなく [`showcases/std`](showcases/std) に置いてください
- 開発中は対象を絞ったチェックを実行し、PR 前には `./scripts/ci-check.sh ai all` を実行してください
- [`CHANGELOG.md`](CHANGELOG.md) は GitHub Actions で生成されるため、手動編集しないでください

## ライセンス

Apache-2.0 / MIT のデュアルライセンスです。[LICENSE-APACHE](LICENSE-APACHE) と [LICENSE-MIT](LICENSE-MIT) を参照してください。
