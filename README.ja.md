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

fraktor-rs は、Pekko と Proto.Actor に着想を得たアクターモデルを、`no_std` ターゲットとホストランタイムの両方で扱うための、仕様駆動のアクターランタイムです。

組み込み向けの軽量な実行環境から、標準ライブラリを利用できるホスト環境まで、同じ設計思想とワークスペース構成のもとで一貫したアクターモデルを提供することを目指しています。

## 特徴

- ワークスペース全体で `core` / `std` の構造を共有し、同じアクターモデルを組み込み環境とホスト環境の両方で扱えます
- `utils` / `actor` / `persistence` / `remote` / `cluster` / `stream` の 6 つのクレートと、主要な利用シナリオを試せる showcase 群を備えています
- ライフサイクル、supervision、death watch、actor path、remoting、typed/untyped bridge など、Pekko / Proto.Actor 由来の主要な意味論を取り込んでいます
- steering、カスタム dylint ルール、CI スクリプトを含む仕様駆動ワークフローにより、変更を一貫した形で管理できます

## クイックスタート

### 必要なもの

- `rustup`
- Rust toolchain `nightly-2025-12-01`
- フルチェックを実行する場合は `cargo-dylint`、`rustc-dev`、`llvm-tools-preview`

### インストール

```bash
rustup toolchain install nightly-2025-12-01 --component rustfmt --component clippy
git clone git@github.com:j5ik2o/fraktor-rs.git
cd fraktor-rs
```

### 実行

```bash
cargo run -p fraktor-showcases-std --example getting_started
```

### 検証

```bash
cargo test -p fraktor-actor-core-kernel-rs --features "std test-support tokio-executor"
./scripts/ci-check.sh all
```

## ワークスペース構成

ワークスペースは以下のクレートで構成されています。

| クレート | 役割 |
| --- | --- |
| [`modules/utils`](modules/utils) | 可搬性の高い基本プリミティブ、ランタイム補助ユーティリティ、アトミック操作、同期機構、タイマー |
| [`modules/actor`](modules/actor) | ActorSystem、メールボックス、supervision、型付き API、スケジューラ、EventStream |
| [`modules/persistence`](modules/persistence) | Event Sourcing、ジャーナル、スナップショットストア、永続アクター支援 |
| [`modules/remote`](modules/remote) | Remoting 拡張、エンドポイント管理、トランスポートアダプタ、障害検知 |
| [`modules/cluster`](modules/cluster) | メンバーシップ管理、ID 解決、配置、トポロジー、pub-sub、ECS 統合 |
| [`modules/stream`](modules/stream) | アクターシステム上に構築されたリアクティブストリーム |
| [`modules/actor/examples`](modules/actor/examples) | typed event stream、classic timers、classic logging などのアクター利用例 |
| [`showcases/std`](showcases/std) | getting started、request/reply、timers、routing、persistence、remoting、clustering などの統合サンプル |

よく使う実行例:

```bash
# 標準的な showcase
cargo run -p fraktor-showcases-std --example request_reply

# 高度な showcase
cargo run -p fraktor-showcases-std --example remote_messaging --features advanced
```

## ドキュメント

- API ドキュメント: [docs.rs/fraktor-rs](https://docs.rs/fraktor-rs)
- リポジトリ知識ベース: [DeepWiki](https://deepwiki.com/j5ik2o/fraktor-rs)
- 現在の parity レポート:
  - [Actor](docs/gap-analysis/actor-gap-analysis.md)
  - [Remote](docs/gap-analysis/remote-gap-analysis.md)
  - [Cluster](docs/gap-analysis/cluster-gap-analysis.md)
  - [Persistence](docs/gap-analysis/persistence-gap-analysis.md)
  - [Stream](docs/gap-analysis/stream-gap-analysis.md)

## サポート

- Issues: [GitHub Issues](https://github.com/j5ik2o/fraktor-rs/issues)
- 実装ルールの基準: [AGENTS.md](AGENTS.md)

## コントリビューション

- 実装前に、このリポジトリの仕様駆動ワークフローに従って要件と設計を整理してください
- プロジェクト全体のルールは [`.kiro/steering`](.kiro/steering) と [AGENTS.md](AGENTS.md) を参照してください
- PR の前に `./scripts/ci-check.sh all` を実行してください
- runtime、remoting、cluster、stream に影響がある場合は、PR 説明に明記してください

## ライセンス

Apache-2.0 / MIT のデュアルライセンスです。[LICENSE-APACHE](LICENSE-APACHE) と [LICENSE-MIT](LICENSE-MIT) を参照してください。
