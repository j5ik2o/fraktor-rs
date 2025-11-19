# fraktor-rs

[![CI](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/fraktor-rs.svg)](https://crates.io/crates/fraktor-rs)
[![docs.rs](https://docs.rs/fraktor-rs/badge.svg)](https://docs.rs/fraktor-rs)
[![Renovate](https://img.shields.io/badge/renovate-enabled-brightgreen.svg)](https://renovatebot.com)
[![dependency status](https://deps.rs/repo/github/j5ik2o/fraktor-rs/status.svg)](https://deps.rs/repo/github/j5ik2o/fraktor-rs)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![License](https://img.shields.io/badge/License-APACHE2.0-blue.svg)](https://opensource.org/licenses/apache-2-0)
[![Lines of Code](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/j5ik2o/fraktor-rs/refs/heads/main/.github/badges/tokei_badge.json)](https://github.com/j5ik2o/fraktor-rs)

> 英語版は [README.md](README.md) を参照してください。

fraktor-rs は Akka/Pekko と protoactor-go のライフサイクル／監視／Remoting パターンを `no_std` 環境と Tokio などのホスト環境に同一 API で提供する、仕様駆動型のアクターランタイムです。ワークスペースは `fraktor-utils-rs`・`fraktor-actor-rs`・`fraktor-remote-rs` の 3 クレートで構成され、旧来の `*-core` / `*-std` クレート分割は廃止されました。各クレートは内部に `core`（`#![no_std]`）と `std` モジュールを持ち、feature で同一 API を切り替えます。

## 主な機能
- **ライフサイクル最優先 ActorSystem** – `modules/actor/src/core/lifecycle/*` と `system/*` が `SystemMessage::{Create,Recreate,Failure}` を system mailbox で優先的に処理し、Deterministic な SupervisorStrategy + DeathWatch を実現します。
- **Typed/Untyped プロトコル橋渡し** – `typed/*` の `Behavior` や `TypedActorRef` と `into_untyped`/`as_untyped` ヘルパにより、Proto.Actor 由来の `reply_to` 流儀を保ちながら sender 依存を排除します。
- **観測性と診断** – EventStream、DeadLetter、LoggerSubscriber、`tick_driver_snapshot` がライフサイクル／Remoting／TickDriver のメトリクスを同一フォーマットで RTT/UART と tracing subscriber へ流し、`RemoteAuthorityManagerGeneric` やスケジューラの挙動を即座に追跡できます。
- **Remoting スタック** – `fraktor-remote-rs::core` は `RemoteActorRefProvider`、`RemoteWatcherDaemon`、`EndpointManager`、Deferred Envelope、Flight Recorder、Quarantine 管理を追加し、Pekko 互換ルールを保ったままアクター階層をノード間に拡張します。
- **トランスポートと障害検知** – `core::loopback_router` によるループバック、`std::transport::tokio_tcp` による TCP ハンドシェイク／バックプレッシャ／`failure_detector` 連携が工場 (`transport::factory`) 経由で差し替え可能です。
- **Toolbox とアロケータ非依存プリミティブ** – `fraktor-utils-rs` が `RuntimeToolbox`、portable atomic、スピンロック、タイマー、Arc 代替を提供し、`thumbv6/v8` MCU とホスト OS が同じ API を共有します。

## アーキテクチャ
```mermaid
flowchart LR
    subgraph Utils [fraktor-utils-rs]
        UC[core (#![no_std])]
        US[std (host helpers)]
    end
    subgraph Actor [fraktor-actor-rs]
        AC[core]
        AS[std + tokio-executor]
    end
    subgraph Remote [fraktor-remote-rs]
        RC[core]
        RS[std + transport]
    end

    UC --> AC
    AC --> RC
    US --> AS
    AS --> RS
    UC --> RS
```

全クレートが `core`/`std` の二層 API を共有します。`core` は割り込み安全な `#![no_std]` 実装を維持し、`std` は Tokio 実行器やログアダプタを後付けします。`fraktor-remote-rs` は actor/utils を合成して Remoting 拡張・Endpoint Registry・Remote Watcher・トランスポート配線を提供します。

## セットアップ
1. **前提ツール**
   - Rust nightly (`rustup toolchain install nightly`)
   - `cargo-dylint` / `rustc-dev` / `llvm-tools-preview`
   - 任意: `thumbv6m-none-eabi`, `thumbv8m.main-none-eabi`
2. **リポジトリ取得**
   ```bash
   git clone git@github.com:j5ik2o/fraktor-rs.git
   cd fraktor-rs
   ```
3. **基本チェック**
   ```bash
   cargo fmt --check
   cargo test -p fraktor-utils-rs
   cargo test -p fraktor-actor-rs --features test-support
   cargo test -p fraktor-remote-rs quickstart --features test-support
   scripts/ci-check.sh all   # lint + dylint + no_std/std/embedded + docs
   ```

## リポジトリ構成
| パス | 説明 |
| --- | --- |
| `modules/utils/` | `fraktor-utils-rs`: `RuntimeToolbox`、portable atomic、タイマー、Arc 代替などのプリミティブ (`core`/`std`)。 |
| `modules/actor/` | `fraktor-actor-rs`: ActorSystem、Mailbox、Supervision、Typed API、Scheduler/TickDriver、EventStream、ActorPath。 |
| `modules/remote/` | `fraktor-remote-rs`: Remoting 拡張、RemoteActorRefProvider、Endpoint Manager/Reader/Writer、Remote Watcher、Loopback/Tokio TCP トランスポート。 |
| `modules/*/examples/` | no_std ping-pong、Tokio 監督、Remoting ループバック/TCP などのサンプル。 |
| `docs/guides/` | ActorSystem 起動、DeathWatch 移行、TickDriver クイックスタートなどの運用ガイド。 |
| `.kiro/steering/` | アーキテクチャ／技術／構造ポリシー。2018 モジュール・1 ファイル 1 型・rustdoc=英語/その他=日本語などを定義。 |
| `.kiro/specs/` | 要件→設計→タスク→実装の OpenSpec ディレクトリ。 |
| `references/` | protoactor-go や Pekko の参照実装スナップショット。 |
| `scripts/` | `ci-check.sh` など lint/dylint/no_std/std/embedded/doc をまとめたスクリプト。 |

## Spec Driven Development
- `/prompts:kiro-spec-init` → `-requirements` → `-design` → `-tasks` で各機能の要求と設計を固めてから実装します。
- 実装は `/prompts:kiro-spec-impl`、検証は `/prompts:kiro-validate-*` で行い、タスクとテストのトレーサビリティを確保します。
- `.kiro/steering/*.md` が 2018 モジュールのみ許可、公開 1 型/ファイル、runtime core での `#[cfg(feature = "std")]` 禁止などの共通ルールを提示します。

## ロードマップ
- `fraktor-remote-rs` の EndpointManager 状態遷移（quarantine 解除、Deferred Envelope 再送、Remote Watcher Daemon）を堅牢化。
- Remoting ループバック／Tokio TCP のクックブックを `docs/guides/remote-*` に掲載。
- Scheduler/TickDriver ガイドに `EventStreamEvent::TickDriver` のメトリクス項目を追加。
- Failure Detector を EventStream Probe と連携させ、Pekko 互換の InvalidAssociation を各トランスポートで観測可能にする。

## コントリビューション
1. `.kiro/specs/<feature>/` に対応する spec を作成し、requirements → design → tasks を完了させてからブランチを切る。
2. Spec のフローに従って実装 (`/prompts:kiro-spec-impl`) を進める。
3. `scripts/ci-check.sh all` で lint/dylint/no_std/std/embedded/doc を通過させる。
4. PR には spec / task ID と runtime・Remoting への影響を明記する。

## ライセンス
Apache-2.0 / MIT のデュアルライセンスです。`LICENSE-APACHE` と `LICENSE-MIT` を参照してください。
