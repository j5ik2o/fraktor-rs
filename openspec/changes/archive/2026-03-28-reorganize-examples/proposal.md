## Why

現在 examples は各モジュール（actor, cluster, stream, persistence, remote）の `examples/` に分散しており、以下の構造的問題がある：

1. **直積爆発** — 同一機能に対して `no_std` / `std` / `tokio` のランタイムバリアント、`typed` / `untyped` の API バリアントが個別ディレクトリとして存在し、60+ example に膨張している。ドメインロジックの差分はほぼゼロで、import とボイラープレートだけが異なる
2. **tick_driver_support の二重化** — `no_std_tick_driver_support.rs` と `std_tick_driver_support.rs` が各モジュールに存在し、同期プリミティブの選択以外のロジックは同一
3. **命名の不統一** — `not_std` / `no_std` / `tokio_std` が混在
4. **ユースケース不在** — 「API のショーケース」になっており、ユーザーが「何を達成したいか」から逆引きできない
5. **モジュール横断 example の不在** — 各モジュール内に閉じており、実用的なユースケース（永続化 + アクター等）を示す場所がない

## What Changes

### A. トップレベル `showcases-std/` 独立クレートの新設

- `showcases-std/Cargo.toml` を workspace member として追加（`publish = false`）
- 基本依存は `features = ["std"]` で宣言
- advanced examples（remote, cluster）向けに `advanced` feature で重い依存（tokio-transport, tokio-executor）を分離
- 共有ユーティリティを `showcases-std/support/` に統合（tick_driver_support を1つに。`src/lib.rs` から `#[path]` で公開）

注: `examples/` ではなく `showcases-std/` を使用する。Cargo はルートパッケージの `examples/` ディレクトリを自動的に example 探索対象とするため、`examples/` を workspace member にすると慣例パスと衝突しビルド分離の前提が崩れる。

### B. ユースケース駆動の12 example に再構成

**Basic examples（actor 系 — typed API）**

| example | ユースケース | 主要モジュール |
|---------|------------|--------------|
| `getting_started` | ActorSystem 起動 → spawn → tell | actor |
| `request_reply` | ask パターン | actor |
| `state_management` | Behavior 切替 + カウンター | actor |
| `child_lifecycle` | spawn + watch + supervision | actor |
| `timers` | once + periodic + cancel | actor |
| `routing` | pool router round-robin | actor |
| `stash` | メッセージの一時退避と復帰 | actor |
| `serialization` | bincode or serde_json | actor |
| `stream_pipeline` | source → map → fold → sink | stream |

**Advanced examples（cross-module — 現行 untyped API を許容）**

| example | ユースケース | 主要モジュール | 追加 feature |
|---------|------------|--------------|-------------|
| `persistent_actor` | イベントソーシング | persistence + actor | — |
| `cluster_membership` | クラスタ参加 | cluster + actor | `advanced` |
| `remote_messaging` | ネットワーク越し通信 | remote + actor | `advanced` |

### C. 既存 examples の削除

- 各モジュール（actor, cluster, stream, persistence, remote）の `examples/` ディレクトリを削除
- 各モジュールの `Cargo.toml` から `[[example]]` セクションを削除

### D. ドキュメント・CI の参照更新

- `README.md` の examples 参照パスを `showcases-std/` に更新
- `docs/guides/getting-started.md` の旧 example パスを新構成に更新
- `scripts/ci-check.sh` の `run_examples()` が新クレートを認識することを確認

## Design Decisions

- **actor 系は typed API、cross-module は現行 API に従う** — actor モジュールは typed API が整備されているため typed のみ提供。persistence / remote / cluster は現行の untyped (core) API がベースであり、typed 化されるまでは現行 API で example を提供する
- **std のみ** — 現状 std 上からの利用が前提。no_std 対応の証明は CI テストで担保
- **対象はユーザー開発者** — モジュール拡張者向け example は将来別途追加
- **ユースケース駆動** — モジュール横断するかは実装詳細であり、ユースケースの達成が目的
- **独立クレート（`showcases-std/`）** — Cargo の `examples/` 慣例パスとの衝突を回避。ルート Cargo.toml の汚染防止、ビルド影響の分離、publish = false の明示
- **依存の feature 分離** — basic examples は軽量な依存のみ。advanced examples は `advanced` feature で tokio-transport / tokio-executor 等の重い依存を隔離し、`cargo run -p fraktor-showcases-std --example getting_started` で不要な依存を引かない

## Future Work

- **`showcases-embedded/`** — no_std / embassy 向けの embedded examples は、ビルドターゲットが根本的に異なるため別クレート `showcases-embedded/` として将来追加する。`showcases-std/` と対称的な命名としている

## Non-Goals

- モジュール拡張者向け example の作成（将来対応）
- no_std / embedded 環境向け example の作成（将来 `showcases-embedded/` で対応）
- ドキュメント（mdBook 等）の作成
