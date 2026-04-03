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

fraktor-rs は、Pekko と Proto.Actor 由来のアクターモデルを `no_std` ターゲットとホストランタイムの両方で扱うための、仕様駆動のアクターランタイムです。utilities、actor、persistence、Remoting、Cluster、Streams を `core` / `std` の共通構造でそろえ、組み込み向け実装と Tokio 向け実装を別コードベースに分断せずに運用できます。

## Highlights

- ワークスペース全体で `core` / `std` の構造を共有し、同じアクターモデルを組み込み環境とホスト環境で扱える
- utils / actor / persistence / remote / cluster / stream の 6 クレートと、主要ユースケースを試せる showcase 群を持つ
- ライフサイクル、supervision、death watch、actor path、remoting、typed/untyped bridge で Pekko / Proto.Actor の意味論を取り込んでいる
- spec-driven workflow、steering、custom dylint、CI スクリプトで変更を一貫して管理できる

## Quickstart

### Requirements

- `rustup`
- Rust toolchain `nightly-2025-12-01`
- フルチェックを回す場合は `cargo-dylint`、`rustc-dev`、`llvm-tools-preview`

### Install

```bash
rustup toolchain install nightly-2025-12-01 --component rustfmt --component clippy
git clone git@github.com:j5ik2o/fraktor-rs.git
cd fraktor-rs
```

### Run

```bash
cargo run -p fraktor-showcases-std --example getting_started
```

### Verify

```bash
cargo test -p fraktor-actor-rs --features "std test-support tokio-executor"
./scripts/ci-check.sh all
```

## Usage

ワークスペースは次のクレートで構成されています。

| クレート | 役割 |
| --- | --- |
| [`modules/utils`](modules/utils) | portable primitives、runtime toolbox、atomics、同期、timer |
| [`modules/actor`](modules/actor) | ActorSystem、mailbox、supervision、typed API、scheduler、EventStream |
| [`modules/persistence`](modules/persistence) | event sourcing、journal、snapshot store、persistent actor support |
| [`modules/remote`](modules/remote) | Remoting 拡張、endpoint 管理、transport adapter、failure detection |
| [`modules/cluster`](modules/cluster) | membership、identity lookup、placement、topology、pub-sub、ECS 統合 |
| [`modules/stream`](modules/stream) | アクターシステム上に構築されたリアクティブストリーム |
| [`modules/actor/examples`](modules/actor/examples) | typed event stream、classic timers、classic logging などの actor 例 |
| [`showcases/std`](showcases/std) | getting started、request/reply、timers、routing、persistence、remoting、clustering などの end-to-end 実行例 |

よく使う実行例:

```bash
# 標準 showcase
cargo run -p fraktor-showcases-std --example request_reply

# advanced showcase
cargo run -p fraktor-showcases-std --example remote_messaging --features advanced
```

## Documentation

- API ドキュメント: [docs.rs/fraktor-rs](https://docs.rs/fraktor-rs)
- リポジトリ知識ベース: [DeepWiki](https://deepwiki.com/j5ik2o/fraktor-rs)
- 現在の parity レポート:
  - [Actor](docs/gap-analysis/actor-gap-analysis.md)
  - [Remote](docs/gap-analysis/remote-gap-analysis.md)
  - [Cluster](docs/gap-analysis/cluster-gap-analysis.md)
  - [Persistence](docs/gap-analysis/persistence-gap-analysis.md)
  - [Stream](docs/gap-analysis/stream-gap-analysis.md)

## Getting help

- Issues: [GitHub Issues](https://github.com/j5ik2o/fraktor-rs/issues)
- 実装ルールの基準: [AGENTS.md](AGENTS.md)

## Contributing

- 実装前に、このリポジトリで採用している spec-driven workflow に従って要求と設計を固めてください
- プロジェクト全体のルールは [`.kiro/steering`](.kiro/steering) と [AGENTS.md](AGENTS.md) を参照してください
- PR 前に `./scripts/ci-check.sh all` を実行してください
- runtime / remoting / cluster / stream への影響がある場合は PR 説明に明記してください

## License

Apache-2.0 / MIT のデュアルライセンスです。[LICENSE-APACHE](LICENSE-APACHE) と [LICENSE-MIT](LICENSE-MIT) を参照してください。
