## Context

fraktor-rs の examples は各モジュールに分散しており、ランタイムバリアント × API バリアントの直積で 60+ に膨張している。ユーザー開発者にとって「何を達成したいか」から example を見つけることが困難な状態。

現在の構造:

```
modules/actor/examples/       ← 46 examples (no_std/std/typed/untyped の組合せ)
modules/cluster/examples/     ← 10 examples
modules/stream/examples/      ← 6 examples
modules/persistence/examples/ ← 2 examples
modules/remote/examples/      ← 2 examples
modules/utils/examples/       ← 1 file
```

## Goals / Non-Goals

**Goals:**

- ユーザー開発者がユースケースから example を逆引きできる構造にする
- 60+ → 12 example に削減し、各 example の目的を明確にする
- 共有ボイラープレート（tick_driver_support）を DRY にする
- `cargo run -p fraktor-showcases-std --example <name>` で統一的に実行可能にする
- basic examples では重い依存（tokio-transport 等）を引かない

**Non-Goals:**

- モジュール拡張者向け example の提供
- no_std / untyped API の example 提供（actor 系）
- example 用の独自フレームワーク/マクロの開発

## Design

### ディレクトリ構造

```
showcases-std/
├── Cargo.toml
├── src/
│   └── lib.rs               ← pub mod support を公開
├── support/
│   ├── mod.rs
│   └── tick_driver.rs        ← 統合版（std::Arc<Mutex> ベース）
├── getting_started/
│   └── main.rs
├── request_reply/
│   └── main.rs
├── state_management/
│   └── main.rs
├── child_lifecycle/
│   └── main.rs
├── timers/
│   └── main.rs
├── routing/
│   └── main.rs
├── stash/
│   └── main.rs
├── serialization/
│   └── main.rs
├── persistent_actor/
│   └── main.rs
├── cluster_membership/
│   └── main.rs
├── stream_pipeline/
│   └── main.rs
└── remote_messaging/
    └── main.rs
```

注: `examples/` ではなく `showcases-std/` を使用する。Cargo はルートパッケージの `examples/` ディレクトリを自動的に example 探索対象とするため、workspace member として使うと慣例パスと衝突する。将来 no_std / embassy 向け examples が必要になった場合は、別クレート `showcases-embedded/` として追加する（ビルドターゲットが根本的に異なるため）。

### Cargo.toml 設計

```toml
[package]
name = "fraktor-showcases-std"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
# 基本依存（全 example 共通）
fraktor-actor-rs = { path = "../modules/actor", features = ["std", "test-support"] }
fraktor-utils-rs = { path = "../modules/utils", features = ["std"] }
fraktor-stream-rs = { path = "../modules/stream", features = ["std"] }
serde_json = "1.0"
bincode = { version = "2.0.1", features = ["alloc", "serde"] }
serde = { version = "1.0", features = ["derive"] }

# advanced 依存（cross-module examples 用）
fraktor-persistence-rs = { path = "../modules/persistence", features = ["std"], optional = true }
fraktor-remote-rs = { path = "../modules/remote", features = ["std", "tokio-transport", "tokio-executor", "test-support"], optional = true }
fraktor-cluster-rs = { path = "../modules/cluster", features = ["std", "test-support"], optional = true }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "net", "sync", "io-util"], optional = true }
anyhow = { version = "1", optional = true }

[features]
default = []
advanced = ["dep:fraktor-persistence-rs", "dep:fraktor-remote-rs", "dep:fraktor-cluster-rs", "dep:tokio", "dep:anyhow"]

[lib]
name = "fraktor_showcases_std"
path = "src/lib.rs"

# --- Basic examples ---

[[example]]
name = "getting_started"
path = "getting_started/main.rs"

[[example]]
name = "request_reply"
path = "request_reply/main.rs"

[[example]]
name = "state_management"
path = "state_management/main.rs"

[[example]]
name = "child_lifecycle"
path = "child_lifecycle/main.rs"

[[example]]
name = "timers"
path = "timers/main.rs"

[[example]]
name = "routing"
path = "routing/main.rs"

[[example]]
name = "stash"
path = "stash/main.rs"

[[example]]
name = "serialization"
path = "serialization/main.rs"

[[example]]
name = "stream_pipeline"
path = "stream_pipeline/main.rs"

# --- Advanced examples (require `advanced` feature) ---

[[example]]
name = "persistent_actor"
path = "persistent_actor/main.rs"
required-features = ["advanced"]

[[example]]
name = "cluster_membership"
path = "cluster_membership/main.rs"
required-features = ["advanced"]

[[example]]
name = "remote_messaging"
path = "remote_messaging/main.rs"
required-features = ["advanced"]
```

### workspace 統合

ルート `Cargo.toml` の `[workspace]` members に追加:

```toml
members = [
    "modules/utils",
    "modules/actor",
    "modules/persistence",
    "modules/remote",
    "modules/cluster",
    "modules/stream",
    "showcases-std",      # ← 追加
]
```

### tick_driver_support 統合

現状の `no_std_tick_driver_support.rs` と `std_tick_driver_support.rs` は同期プリミティブの選択のみが異なる。std 環境専用のため `std::sync::Arc<Mutex>` ベースの単一実装に統合する。

```rust
// showcases-std/support/tick_driver.rs
// std 版のみ。Arc<Mutex> ベースの統合実装。
// 既存の std_tick_driver_support.rs をベースとする。
```

### support の参照方式

`src/lib.rs` で support モジュールを公開し、各 example から通常の `use` で参照する:

```rust
// showcases-std/src/lib.rs
#[path = "../support/mod.rs"]
pub mod support;
```

```rust
// 各 example の main.rs から
use fraktor_showcases_std::support::tick_driver;
```

注: `src/lib.rs` は support の公開のみを担う薄いモジュール。example 間で共有するユーティリティの参照先を一元化し、`#[path]` 属性を各 example に分散させない。

### API 方針

| カテゴリ | API | 理由 |
|----------|-----|------|
| Basic examples（actor, stream 系） | typed API のみ | typed API が整備済み。ユーザー開発者には型安全な API を提供 |
| Advanced examples（persistence, remote, cluster） | 現行 API（untyped core） | 現時点で typed API が未整備。既存の動作する untyped 実装をベースとする |

Advanced examples は将来 typed API が整備された時点で typed に移行する。

### 各 example の設計方針

1. **1 example = 1 ユースケース** — 複数概念を混ぜない
2. **上から下に読める** — `main()` の冒頭にユースケースの説明を `//!` doc comment で記述
3. **最小構成** — ユースケース達成に必要最小限のコード。余計な import や構造体を含めない
4. **日本語コメント** — rustdoc 以外のコメントは日本語（プロジェクト規約）
5. **実行可能** — basic: `cargo run -p fraktor-showcases-std --example <name>` / advanced: `cargo run -p fraktor-showcases-std --features advanced --example <name>` で動作確認できる

### example 内容概要

| example | 概要 | 主要 API | カテゴリ |
|---------|------|----------|----------|
| `getting_started` | ActorSystem 起動、guardian spawn、tell でメッセージ送信 | `TypedActorSystem`, `Behaviors::receive_message`, `tell` | Basic |
| `request_reply` | ask パターンで応答を受け取る | `ask`, `TypedActorRef` | Basic |
| `state_management` | カウンターアクター、Behavior 切替で状態遷移 | `Behaviors::receive_message`, `Behaviors::same()`, 新しい `Behavior` を返す関数型遷移 | Basic |
| `child_lifecycle` | 子アクター spawn、watch、Terminated シグナル、supervision | `Behaviors::setup`, `spawn_child`, `watch`, `BehaviorSignal::Terminated`, `SupervisorStrategy` | Basic |
| `timers` | 遅延実行（once）、定期実行（periodic）、キャンセル | `Scheduler`, `TimerKey`, `cancel` | Basic |
| `routing` | Pool Router による round-robin 負荷分散 | `Routers::pool(...).with_round_robin().build()` | Basic |
| `stash` | メッセージの一時退避と復帰 | `Behaviors::with_stash`, `StashBuffer`, `stash`, `unstash_all` | Basic |
| `serialization` | メッセージの serde_json / bincode シリアライゼーション | `Serializer`, `SerializedMessage`, `SerializerWithStringManifest` | Basic |
| `stream_pipeline` | Source → Map → Fold → Sink のデータパイプライン | `Source`, `Flow`, `Sink`, `Materializer` | Basic |
| `persistent_actor` | イベントソーシングによるアクター状態永続化 | `PersistentActor`, `persistent_props`, `spawn_persistent`, `InMemoryJournal` | Advanced |
| `cluster_membership` | クラスタへの参加とメンバーシップ変更の観測 | `MembershipTable`, `VirtualActorRegistry`, `ClusterProvider` | Advanced |
| `remote_messaging` | ネットワーク越しのアクター通信 | `RemotingExtensionInstaller`, `RemotingExtensionConfig`, `default_loopback_setup` | Advanced |

### CI 影響

- `scripts/ci-check.sh` の `run_examples()` は `cargo metadata` で example 一覧を取得し、各 example の `required-features` を読んで `cargo run --features ...` を自動付与する（ci-check.sh:1166-1195）。そのため `showcases-std` が workspace member に含まれていれば、basic / advanced 両方の example が自動的に認識・実行される
- **未確定事項**: `integration-test` コマンド（ci-check.sh:1049-1054）は現在 `--features test-support` 固定で example をビルドしている。advanced examples を `integration-test` の対象にも含めるかどうかは、実装フェーズ（T16）で判断する
- dylint lint は showcases-std には適用しない（`publish = false` かつユーザー向けコード）
- basic examples のビルド確認: `cargo build -p fraktor-showcases-std --examples`
- advanced examples のビルド確認: `cargo build -p fraktor-showcases-std --features advanced --examples`

## Alternatives Considered

### ルートクレートの examples/ を使う（案却下）

ルート `Cargo.toml` に `[[example]]` を大量追加する必要があり、dev-dependencies がルートに混入する。ビルド影響の分離ができないため却下。

### 各モジュール内に残す（案却下）

モジュール横断 example の配置場所がなく、ユーザーがユースケースから探せない。現状の問題が解決しないため却下。

### `examples/` をディレクトリ名として使う（案却下）

Cargo はルートパッケージの `examples/` ディレクトリを自動的に example 探索対象とする。`autoexamples = false` をルートに設定しても、workspace 内の他パッケージとの混同リスクがある。慣例パスを避けて `showcases-std/` を採用。`-std` サフィックスは将来の `showcases-embedded/` との対称性を考慮した命名。
