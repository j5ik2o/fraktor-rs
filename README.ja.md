# fraktor-rs

[![ci](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/fraktor-rs.svg)](https://crates.io/crates/fraktor-rs)
[![docs.rs](https://docs.rs/fraktor-rs/badge.svg)](https://docs.rs/faktor-rs)
[![Renovate](https://img.shields.io/badge/renovate-enabled-brightgreen.svg)](https://renovatebot.com)
[![dependency status](https://deps.rs/repo/github/j5ik2o/fraktor-rs/status.svg)](https://deps.rs/repo/github/j5ik2o/fraktor-rs)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![License](https://img.shields.io/badge/License-APACHE2.0-blue.svg)](https://opensource.org/licenses/apache-2-0)
[![Lines of Code](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/j5ik2o/fraktor-rs/refs/heads/main/.github/badges/tokei_badge.json)](https://github.com/j5ik2o/fraktor-rs)

> 英語版は [README.md](README.md) を参照してください。

fraktor-rs は Akka/Pekko・protoactor-go のライフサイクル／監視／Remoting 流儀を Rust の `no_std`/標準環境へ共通 API で落とし込むアクターランタイムです。`utils-*` と `actor-*` クレートを縦に積み、RP2040 などのマイコンから Tokio を使うホスト OS まで一貫したデプロイを実現します。

## 主な特徴
- **ライフサイクル最優先**: `SystemMessage::{Create,Recreate,Failure}` が system mailbox を先取し、SupervisorStrategy + DeathWatch の決定性を担保。
- **Pekko 互換 ActorPath**: `ActorPathParts`, `PathSegment`, `ActorPathFormatter` が guardian 自動挿入・UID サフィックス付きの `pekko://system@host:port/user/...` URI を生成し、RFC2396 準拠パーサと連携。
- **Remote authority 管理**: `RemoteAuthorityManager` が `Unresolved → Connected → Quarantine` を管理し、`RemotingConfig` 由来の隔離期間と Deferred キューを同期。
- **観測性**: EventStream/DeadLetter/LoggerSubscriber が RTT/UART からホスト側 tracing まで低レイテンシで転送。
- **Typed/Untyped 並存**: `TypedActor` が `into_untyped`/`as_untyped` で Classic API と相互運用し、`reply_to` で sender-free な設計を維持。
- **RuntimeToolbox 抽象**: `fraktor-utils-core` が portable atomic・スピンロック・タイマーファミリを提供し、上位レイヤーは割り込み安全かつアロケータ非依存に実装。

## アーキテクチャ
```
utils-core  -->  actor-core  -->  actor-std
   ^              ^                ^
   |              |                |
ユーティリティ   ActorSystem      Tokio/ホスト適合
```
- `utils-core`: Portable atomic, SmallVec 互換 Primitive, RFC2396 URI パーサなどの共通下支え。
- `actor-core`: no_std ActorSystem, actor path registry, RemoteAuthorityManager, supervision, mailbox。
- `actor-std`: Tokio 実行器やログ/トレース連携などホスト専用の橋渡し。
- `utils-std`: 標準環境専用の補助ユーティリティ。

## はじめかた
1. **前提ツール**: Rust nightly / `cargo-dylint` / `rustc-dev` / `llvm-tools-preview`。組込み向けは `thumbv6m` `thumbv8m.main` ターゲットも追加。
2. **セットアップ**:
   ```bash
   git clone git@github.com:j5ik2o/fraktor-rs.git
   cd fraktor-rs
   ```
3. **基本チェック**:
   ```bash
   cargo fmt --check
   cargo test -p fraktor-utils-core-rs uri_parser
   cargo test -p fraktor-actor-core-rs actor_path
   scripts/ci-check.sh all
   ```

## リポジトリ構成
| パス | 説明 |
| --- | --- |
| `modules/utils-core` | RuntimeToolbox、portable atomic、URI パーサ等の no_std 基盤 |
| `modules/actor-core` | ActorSystem/ActorPath/RemoteAuthority 等の中核 |
| `modules/actor-std` | Tokio 実行器やホスト向けログ基盤 |
| `modules/utils-std` | 標準環境専用ユーティリティ |
| `scripts/` | `ci-check.sh` など CI フローの入口 |
| `.kiro/` | OpenSpec (requirements/design/tasks) と steering ガイドライン |

## Spec Driven Development
1. `.kiro/specs/<feature>/requirements.md → design.md → tasks.md` の順に `/prompts:kiro-*` で承認。
2. `.kiro/steering/*` が 2018 モジュール構成・1 ファイル 1 型・rustdoc=英語/その他=日本語 といった共通ルールを定義。
3. 実装後は `/prompts:kiro-validate-*` でタスク完了・テスト網羅性をチェック。

## ロードマップ例
- ActorSelection resolver の `..`／guardian 境界検証を強化。
- RemoteAuthorityManager のタイマー・Deferred キューを完成させ、Remoting 実装の足場を固める。
- `docs/guides` を bilingual 化し、移行ガイドを充実。

## コントリビュート手順
1. フォーク＋ブランチ作成。
2. `.kiro/specs/` で要求/設計/タスクを更新し、承認後にコードを変更。
3. `scripts/ci-check.sh all` で lint/dylint/no_std/std/embedded/doc を通過させる。
4. PR では紐づく spec/taks を明記。

## ライセンス
Apache-2.0 / MIT のデュアルライセンスです (`LICENSE-APACHE`, `LICENSE-MIT`)。
