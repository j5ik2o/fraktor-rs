# remote-adaptor-std-package Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: クレート存在と命名

新クレート `fraktor-remote-adaptor-std-rs` が `modules/remote-adaptor-std/` ディレクトリに存在し、ワークスペースの `Cargo.toml` の `members` に登録されている SHALL。

#### Scenario: クレートのビルド成功

- **WHEN** ワークスペースルートで `cargo build -p fraktor-remote-adaptor-std-rs` を実行する
- **THEN** クレートが警告なしでビルドされる

#### Scenario: クレート命名の一貫性

- **WHEN** `modules/remote-adaptor-std/Cargo.toml` を読む
- **THEN** `name = "fraktor-remote-adaptor-std-rs"` と記載されている

### Requirement: core クレート依存

クレートは `fraktor-remote-core-rs` に依存し、core の trait / 型を実装する SHALL。

#### Scenario: core 依存の存在

- **WHEN** `modules/remote-adaptor-std/Cargo.toml` の `[dependencies]` セクションを検査する
- **THEN** `fraktor-remote-core-rs` がエントリとして存在する

#### Scenario: actor-core 依存の存在

- **WHEN** `modules/remote-adaptor-std/Cargo.toml` の `[dependencies]` セクションを検査する
- **THEN** `fraktor-actor-core-rs` と `fraktor-actor-adaptor-rs` がエントリとして存在する (loopback 振り分けで local actor ref provider を呼ぶため)

### Requirement: std + tokio 前提

クレートは std + tokio を前提とし、no_std 制約は適用されない SHALL。

#### Scenario: lib.rs の std 許可

- **WHEN** `modules/remote-adaptor-std/src/lib.rs` を読む
- **THEN** `#![no_std]` 属性は存在せず、std の利用が許可されている (ただし他の `*-adaptor-std` クレート同様の属性パターンは維持する)

#### Scenario: tokio 依存の存在

- **WHEN** `modules/remote-adaptor-std/Cargo.toml` の `[dependencies]` セクションを検査する
- **THEN** `tokio` が (rt-multi-thread, net, sync, time, io-util の features で) 依存に含まれている

### Requirement: モジュール構成

クレートは以下のサブモジュールを `src/` 配下に持つ SHALL: `tcp_transport`、`association`、`watcher_actor`、`provider`、`extension_installer`。

#### Scenario: 必須サブモジュールの存在

- **WHEN** `modules/remote-adaptor-std/src/` のディレクトリ一覧を取得する
- **THEN** `tcp_transport.rs`、`association.rs`、`watcher_actor.rs`、`provider.rs`、`extension_installer.rs` および対応するディレクトリが存在する

### Requirement: ライセンスとメタデータ

クレートは他モジュールと同じライセンス (`MIT OR Apache-2.0`) を持ち、`description`・`homepage`・`repository`・`documentation`・`keywords`・`categories` を `Cargo.toml` に記載する SHALL。

#### Scenario: メタデータの完備

- **WHEN** `modules/remote-adaptor-std/Cargo.toml` を読む
- **THEN** `description`・`license`・`homepage`・`repository`・`documentation`・`keywords`・`categories`・`edition = "2024"` がすべて記載されている
