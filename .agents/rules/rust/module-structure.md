# fraktor-rs モジュール構造（core / std 分離）

## 原則

**各モジュールの `src/` は `core/`（no_std）と `std/`（アダプタ）に分離する。**

`core/` がポートを定義し、`std/` がそれを実装する依存方向を維持すること。

## ディレクトリ構造

```
modules/{name}/src/
├── core/      # no_std コアロジック・ポート（trait）定義
├── core.rs    # core モジュールの宣言ファイル
├── std/       # std/tokio アダプタ（core のポートを実装）
├── std.rs     # std モジュールの宣言ファイル
├── embedded/  # 組み込み向け（embassy）アダプタ（将来配置予定）
├── embedded.rs
└── lib.rs
```

## 各層の責務

| 層 | パス | no_std | 役割 |
|----|------|--------|------|
| core | `modules/{name}/src/core/` | ✅ | コアロジック・ポート（trait）定義 |
| std | `modules/{name}/src/std/` | ❌ | std/tokio を使ったアダプタ実装。core に依存する |
| embedded | `modules/{name}/src/embedded/` | ✅ | 組み込み向け（embassy）アダプタ実装。core に依存する。将来配置予定 |

## core 内部の層構造

core 内部は機能別サブモジュールがフラットに並ぶ。
actor モジュールのように `typed/` サブ層を持つ場合がある。

| サブ層 | パス例 | 役割 |
|--------|--------|------|
| untyped kernel | `core/actor/`, `core/dispatch/`, `core/supervision/` | 型パラメータなしのコアロジック。`dyn Any` ベースのメッセージング。ロジックはここに集約する |
| typed ラッパー | `core/typed/` | untyped kernel を型安全にラップ。`Behavior<M>`, `TypedActorRef<M>` 等。できるだけ薄く保ち、ロジックを持たせない |

**注意**: すべてのモジュールが `typed/` サブ層を持つわけではない。

## 依存方向

```
std/（アダプタ）  embedded/（アダプタ）
   │                  │
   │  依存可（↓のみ）  │
   ▼                  ▼
   core/（コアロジック・ポート定義）
```

- `std/` と `embedded/` は `core/` に依存してよい
- `core/` は `std/` や `embedded/` に依存してはならない（no_std 制約が壊れる）
- `std/` と `embedded/` は互いに依存してはならない

## コード例

```rust
// ✅ core/: no_std でポートを定義
// modules/actor/src/core/dispatch/mailbox.rs
pub trait Mailbox {
    fn enqueue(&mut self, msg: Message);
}

// ✅ std/: core のポートを std/tokio で実装
// modules/actor/src/std/dispatch/mailbox.rs
use crate::core::dispatch::Mailbox;

pub struct TokioMailbox { /* ... */ }

impl Mailbox for TokioMailbox {
    fn enqueue(&mut self, msg: Message) { /* tokio 実装 */ }
}

// ❌ WRONG: core/ が std/ に依存
// modules/actor/src/core/dispatch/mailbox.rs
use crate::std::dispatch::TokioMailbox;  // 禁止
```

## 禁止パターン

- `core/` 内で `std`, `tokio`, `async-std` 等を直接 `use`（no_std 制約違反）
- `core/` 内で `std/` のモジュールを参照（依存方向の逆転）
- `#![no_std]` 制約を `#[cfg(feature = "std")]` で安易に迂回
- `std/` に no_std でも動くロジックを置く（`core/` に移動すべき）

## モジュール一覧

| モジュール | パス |
|------------|------|
| actor | `modules/actor/` |
| cluster | `modules/cluster/` |
| persistence | `modules/persistence/` |
| remote | `modules/remote/` |
| streams | `modules/streams/` |
| utils | `modules/utils/` |
