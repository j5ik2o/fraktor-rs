## Why

`actor-core` は no_std クレートでありプラットフォーム固定ではない。共有ロック構築は `utils-core` が提供する `SharedLock + DefaultMutex` 抽象（feature による backend 切り替えを前提）を通すというガバナンス（`actor-lock-construction-governance`）が既に確立されている。

ところが `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tick_feed.rs` の 1 ファイルだけが、この抽象を素通りして `critical_section::Mutex<RefCell<VecDeque<u32>>>` を直接 use している。これにより:

1. `actor-core/Cargo.toml:24` に `critical-section` クレートへの **non-optional な直接依存** が宣言されている
2. `actor-core/Cargo.toml:19` の `test-support = ["critical-section/std"]` の存在理由の片方（impl provider 提供）を生み続けている
3. ガバナンス（`actor-lock-construction-governance` および `compile-time-lock-backend`）が lock backend 固定指定を禁じている文脈に対し、`critical-section` 直接 use は文言上カバーされておらず、将来の同種逸脱を予防する明示的要件が欠けている

「源を絶つ」とは、`actor-core` の **production code（ソースコード）から `critical-section` クレートへの直接利用を撤去** すること。Cargo の `<dep>/<feature>` 構文制約により、`test-support = ["critical-section/std"]` を維持するためには `[dependencies]` の `critical-section` エントリ自体は完全削除できず **`optional = true` で残す** 必要がある（詳細は design.md Decision 6）。`portable-atomic` 経由の間接依存と `test-support` feature 自体の撤去は別 change で扱う。

## What Changes

- `tick_feed.rs` の `queue: critical_section::Mutex<RefCell<VecDeque<u32>>>` を `queue: SharedLock<VecDeque<u32>>` に置換し、driver は `DefaultMutex` で feature 切り替えに任せる
- `tick_feed.rs` 内の `critical_section::with(|cs| ...)` を `SharedLock::with_lock(|q| ...)` に置換し、`RefCell` 二重ラップを除去
- `TickFeed` 構造体の内部フィールド `queue` の型変更（フィールドは `pub` ではないため public API には影響しない）。`new`、`enqueue`、`enqueue_from_isr`、`drain_pending`、`snapshot` 等のメソッドシグネチャは不変
- `actor-core/Cargo.toml:24` の `critical-section = { workspace = true, default-features = false }` を `critical-section = { workspace = true, default-features = false, optional = true }` に変更（optional 化）。これにより `actor-core` の通常ビルドでは引き込まれず、ソースコードから直接 use されることもない
- `actor-core/Cargo.toml:19` の `test-support = ["critical-section/std"]` を `test-support = ["dep:critical-section", "critical-section/std"]` に変更。Cargo features 制約に対応しつつ impl provider 提供の機能性を維持する
- `actor-lock-construction-governance` spec を拡張し、以下の 2 要件を追加する。これにより同種の逸脱を将来的に lint または review で検出可能にする
  - **要件 A**: `actor-*` の production code は primitive lock crate（`critical-section`、`spin`、`parking_lot` 等）および `std::sync::Mutex` / `std::sync::RwLock` を直接 use / 構築してはならない（shared state は `SharedLock + DefaultMutex` を経由する）
  - **要件 B**: `actor-*` の `Cargo.toml` は primitive lock crate を `[dependencies]` に **non-optional な直接依存** として宣言してはならない（推移的依存、または `optional = true` かつ feature で gated された impl provider 用エントリ経由でのみ表現する）

## Capabilities

### New Capabilities

なし。

### Modified Capabilities

- `actor-lock-construction-governance`: 既存ガバナンス（backend concrete / fixed-family alias の直接構築禁止）に対し、以下 2 要件を追加する
  - 要件 A: primitive lock crate および `std::sync::Mutex` / `std::sync::RwLock` の直接 use / 構築禁止（コードレベル）
  - 要件 B: primitive lock crate の `[dependencies]` への non-optional 直接宣言禁止（依存管理レベル。`optional = true` かつ feature で gated された impl provider 用エントリは例外）

## Impact

### 影響を受けるコード
- `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tick_feed.rs`（実装置換）
- `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tests.rs`（`enqueue_from_isr` 関連テストの動作確認のみ。`TickFeed` を直接構築している箇所があれば追従修正）
- `modules/actor-core/Cargo.toml`（`critical-section` の optional 化、`test-support` feature の `dep:critical-section` 追加）
- `openspec/specs/actor-lock-construction-governance/spec.md`（apply 時に 2 要件が merge される）

### 影響を受けない範囲
- `tick_feed.rs` の public API シグネチャ（`enqueue`、`enqueue_from_isr`、`drain_pending`、`snapshot`、`signal`、`handle`、`driver_active`、`new`、`set_resolution`）
- `test-support` feature を有効化したときの impl provider 提供機能（実装は `dep:critical-section` 追加で内部表現が変わるが、外部から見た挙動は不変）
- 他クレート（`fraktor-utils-core-rs`、`fraktor-actor-adaptor-std-rs`、`fraktor-cluster-*-rs`、`fraktor-remote-*-rs`、`fraktor-stream-*-rs`、`fraktor-persistence-*-rs`）

### 依存関係
- `actor-core` の通常ビルドから `critical-section` クレートへの依存が消える（optional 化により feature 無効時は引き込まれない）
- `test-support` feature 有効時のみ `critical-section` クレートが直接依存として有効化される（impl provider 用）
- 推移的依存としての `critical-section` は `portable-atomic` 経由で残る（今回は触らない）

### リスク
- **ISR セーフティ表面の変化**: `critical_section::Mutex` は割り込み禁止区間で動作する設計だが、`enqueue_from_isr` の caller を grep した結果、production caller は存在せず actor-core 内テストコード（`modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tests.rs:83-84`）のみが呼んでいる。よって ISR 安全性の実装上の意味は現状なし。詳細は design.md Decision 3 で評価
- 既存 spec `actor-lock-construction-governance` への要件追加により、将来の同種違反を CI/lint で検出可能になるが、現存コードに新たな違反が見つかる可能性がある（修正は別 change で対応）
