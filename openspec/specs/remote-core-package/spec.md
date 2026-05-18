# remote-core-package Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: クレート存在と命名

新クレート `fraktor-remote-core-rs` が `modules/remote-core/` ディレクトリに存在し、ワークスペースの `Cargo.toml` の `members` に登録されている SHALL。

#### Scenario: クレートのビルド成功

- **WHEN** ワークスペースルートで `cargo build -p fraktor-remote-core-rs` を実行する
- **THEN** クレートが警告なしでビルドされる

#### Scenario: クレート命名の一貫性

- **WHEN** `modules/remote-core/Cargo.toml` を読む
- **THEN** `name = "fraktor-remote-core-rs"` と記載されている

### Requirement: no_std 制約の機械的強制

クレートは `lib.rs` で **`#![cfg_attr(not(test), no_std)]` または同等の no_std 条件** を宣言し、プロダクションコードでは `std`・`tokio`・`async-std`・`tokio_util` 等の async runtime や std-only クレートに依存しない SHALL。`alloc` クレートは利用可能であり、`#[cfg(test)]` では std を使える。

#### Scenario: no_std build の成功

- **WHEN** `cargo build -p fraktor-remote-core-rs --no-default-features` を実行する
- **THEN** ビルドが成功する

#### Scenario: tokio 依存の不在

- **WHEN** `modules/remote-core/Cargo.toml` の `[dependencies]` セクションを検査する
- **THEN** `tokio`・`async-std`・`async-trait`・`futures` のいずれもエントリとして存在しない

#### Scenario: std 直接 use の不在 (プロダクションコード)

- **WHEN** `modules/remote-core/src/` 配下のすべての `.rs` ファイルを検査する。ただし `#[cfg(test)]` ブロック内および `#[cfg(any(test, feature = "test-support"))]` 配下は除外する
- **THEN** `use std::` および `use ::std::` を含む行が存在しない (`extern crate alloc` および `use core::` / `use alloc::` は許可)

#### Scenario: テストコードは std を利用可能

- **WHEN** `#[cfg(test)]` ブロックまたは `tests/` 配下のテストコードを検査する
- **THEN** これらのテストコードが `use std::` を含むことは許可される (他クレート同様の扱い)

### Requirement: 機能ゲートの不在

クレートには `tokio-transport` 系の transport 実装ゲートを含む `#[cfg(feature = "...")]` を、no_std 制約と無関係な目的で使用しない SHALL。`#[cfg(test)]` および `#[cfg(feature = "test-support")]` のみ許可。

#### Scenario: tokio-transport feature の不在

- **WHEN** `modules/remote-core/Cargo.toml` の `[features]` セクションを検査する
- **THEN** `tokio-transport`・`std-runtime` 等の transport 実装をゲートする feature が存在しない

#### Scenario: 不適切な cfg gate の不在

- **WHEN** `modules/remote-core/src/` 配下を `#[cfg(feature` でgrep する
- **THEN** マッチするのは `test-support` のみで、`tokio-transport` 等の transport 系は0件である

### Requirement: モジュール構成

クレートは Pekko Artery の責務分離に対応する以下のサブモジュールを `src/` 配下に持つ SHALL: `address`、`settings`、`wire`、`envelope`、`association`、`failure_detector`、`watcher`、`instrument`、`transport`、`provider`、`extension`。

#### Scenario: 必須サブモジュールの存在

- **WHEN** `modules/remote-core/src/` のディレクトリ一覧を取得する
- **THEN** `address.rs`、`settings.rs`、`wire.rs`、`envelope.rs`、`association.rs`、`failure_detector.rs`、`watcher.rs`、`instrument.rs`、`transport.rs`、`provider.rs`、`extension.rs` および対応するディレクトリが存在する

#### Scenario: lib.rs での宣言

- **WHEN** `modules/remote-core/src/lib.rs` を読む
- **THEN** すべての必須サブモジュールが `pub mod` で宣言されている

### Requirement: ライセンスとメタデータ

クレートは他モジュールと同じライセンス (`MIT OR Apache-2.0`) を持ち、`description`・`homepage`・`repository`・`documentation`・`keywords`・`categories` を `Cargo.toml` に記載する SHALL。

#### Scenario: メタデータの完備

- **WHEN** `modules/remote-core/Cargo.toml` を読む
- **THEN** `description`・`license`・`homepage`・`repository`・`documentation`・`keywords`・`categories`・`edition = "2024"` がすべて記載されている

### Requirement: 旧 modules/remote/ の完全削除 (legacy migration 契約)

本 change (`remote-redesign`) の archive 時点において、旧 `modules/remote/` ディレクトリおよび関連する `fraktor-remote-rs` クレートへの参照は **完全に削除されている** SHALL。これは `legacy-code-temporary-usage.md` ルール3「PRまたはタスク完了時には、同一責務のレガシー実装を残さない」への構造的準拠を担保するための要件であり、新 capability を別途作らず本 capability の一部として migration 契約として扱う。

**位置付け**: 本 Requirement は Phase A〜D の途中では満たされない。Phase E 完了時点 (= change の archive 時点) で初めて満たされる契約である。archive 前の最終チェック (tasks.md Section 29-30) でこの Requirement を満たすことが必須。

#### Scenario: modules/remote/ ディレクトリの不在 (archive 時点)

- **WHEN** 本 change の archive 直前に `ls modules/` を実行する
- **THEN** `remote` ディレクトリは存在しない (`remote-core` と `remote-adaptor-std` は存在する)

#### Scenario: fraktor-remote-rs クレート依存の不在

- **WHEN** 本 change の archive 直前に `grep -rn 'fraktor-remote-rs' modules/*/Cargo.toml` を実行する
- **THEN** 結果はゼロ件である (すべての依存元が `fraktor-remote-core-rs` + `fraktor-remote-adaptor-std-rs` に切り替え済み)

#### Scenario: workspace members からの除外

- **WHEN** 本 change の archive 直前にワークスペースルート `Cargo.toml` の `[workspace] members` を検査する
- **THEN** `modules/remote` エントリは存在しない。`modules/remote-core` と `modules/remote-adaptor-std` は存在する

#### Scenario: 全 build/test 通過

- **WHEN** 旧削除後に `./scripts/ci-check.sh ai all` を実行する
- **THEN** すべての build・test・lint・doc 検査がエラーゼロで通過する (旧クレート削除後も全モジュールが新クレート経由で正常動作することを保証)

#### Scenario: 例外承認の不要性

- **WHEN** 本 change の proposal / design を読む
- **THEN** `legacy-code-temporary-usage.md` の例外承認条項 (「例外が必要な場合は、事前に作業計画に明記し、削除条件を明文化する」) は参照されていない (単一 change 化によりルール3 準拠が構造的に達成されるため、例外承認プロセス自体が不要)

### Requirement: remote-adaptor-std は remote-core の代替 lifecycle 入口を定義してはならない

`fraktor-remote-core-rs` は remote lifecycle の標準意味論として `Remote` / `RemoteShared` / `Remoting` port を提供しなければならない（MUST）。`fraktor-remote-adaptor-std-rs` は `RemoteTransport` の具象実装、actor system 配線、std runtime task orchestration を提供する adapter crate であり、`remote-core::Remote` と競合する lifecycle semantics を public API として定義してはならない（MUST NOT）。std adapter は ActorSystem lifecycle に接続された内部実装として core lifecycle operation を呼び出してよい（MAY）が、通常利用者に別 lifecycle sequence を書かせてはならない（MUST NOT）。

#### Scenario: std adaptor は StdRemoting 相当の public wrapper を持たない

- **WHEN** `modules/remote-adaptor-std/src` 配下の public 型と re-export を検査する
- **THEN** `StdRemoting` または同等の remote lifecycle wrapper は存在しない
- **AND** remote lifecycle の状態遷移と意味論は `remote-core::Remote` / `RemoteShared` に残る
- **AND** user-facing application code は `remote-core::Remote` を startup sequence として直接操作しない

#### Scenario: std adaptor は Port 実装と配線だけを提供する

- **WHEN** 利用者が std 環境で remote lifecycle を開始する
- **THEN** `TcpRemoteTransport` が `RemoteTransport` port 実装として core `RemoteShared` に差し込まれる
- **AND** `RemotingExtensionInstaller` は ActorSystem lifecycle に接続された adapter として core lifecycle operation を内部で呼ぶ
- **AND** `RemotingExtensionInstaller` は `remote.start()` / `spawn_run_task()` / `shutdown_and_join()` を通常利用者が順に呼ぶ別 lifecycle API として提供してはならない

### Requirement: remote-core の公開 API は adapter runtime internal に依存してはならない

`fraktor-remote-core-rs` は `fraktor-remote-adaptor-std-rs` の runtime internal 型に依存してはならない（MUST NOT）。`Remote` は `RemoteTransport` port だけに依存し、std transport や provider bridge の具象型を型シグネチャに露出してはならない（MUST NOT）。

#### Scenario: Remote は TcpRemoteTransport を型パラメータとして露出しない

- **WHEN** `modules/remote-core/src/extension/remote.rs` を検査する
- **THEN** `pub struct Remote<T>` ではなく非ジェネリックな `pub struct Remote` である
- **AND** `Remote` の public method signature に `TcpRemoteTransport` は現れない

#### Scenario: remote-core は remote-adaptor-std に依存しない

- **WHEN** `modules/remote-core/Cargo.toml` の dependencies を検査する
- **THEN** `fraktor-remote-adaptor-std-rs` への依存は存在しない
- **AND** `modules/remote-core/src` 配下に `fraktor_remote_adaptor_std_rs` への import は存在しない
