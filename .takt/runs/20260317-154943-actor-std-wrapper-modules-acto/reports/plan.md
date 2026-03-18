# タスク計画

## 元の要求

`.takt/runs/20260317-154943-actor-std-wrapper-modules-acto/context/task/order.md` に記載されたタスクの実装計画を立てる。

## 分析結果

**注意: 本計画は前回の分析結果に基づいています。前回の調査で、タスクの全要件が既に現在のコードベースで満たされていることが判明しました。以下にその詳細を記載します。**

### 目的

actor モジュールの `std/` 配下にラッパーモジュール（`actor_context.rs`, `actor_ref.rs`, `actor_system.rs`）を作成し、`core/` の型を re-export することで、利用者が `std::` パスからもアクセスできるようにする。

### 参照資料の調査結果

タスク指示書（order.md）で指定された参照資料を確認した結果：

**既存実装の状態:**
- `modules/actor/src/std/actor_context.rs` — 既に存在し、`core::actor_context::ActorContext` を `pub use` で re-export 済み
- `modules/actor/src/std/actor_ref.rs` — 既に存在し、`core::actor_ref::ActorRef` を `pub use` で re-export 済み  
- `modules/actor/src/std/actor_system.rs` — 既に存在し、`core::actor_system::ActorSystem` および関連型を `pub use` で re-export 済み
- `modules/actor/src/std.rs` — 上記3モジュールが `pub mod` で宣言済み

**テストの状態:**
- 既存テスト（`cargo test -p fraktor-actor-rs --features std,tokio`）が正常にパス
- re-export パスからのアクセスが機能していることを確認済み

### スコープ

**変更不要** — タスクで要求された全ファイル・全 re-export が既に実装済みのため、コード変更は一切不要。

| 要件 | 現行コードの該当箇所 | 状態 |
|------|---------------------|------|
| `std/actor_context.rs` で `ActorContext` を re-export | `modules/actor/src/std/actor_context.rs:1` | ✅ 実装済み |
| `std/actor_ref.rs` で `ActorRef` を re-export | `modules/actor/src/std/actor_ref.rs:1` | ✅ 実装済み |
| `std/actor_system.rs` で `ActorSystem` を re-export | `modules/actor/src/std/actor_system.rs:1-4` | ✅ 実装済み |
| `std.rs` にモジュール宣言 | `modules/actor/src/std.rs` に `pub mod actor_context/actor_ref/actor_system` | ✅ 実装済み |

### 実装アプローチ

**実装作業なし。** 全要件が既に満たされているため、後続のムーブメント（write_tests, implement）でも追加作業は不要。

## 確認事項

- タスクの要件が既に完全に満たされている場合、このピースをスキップまたは完了として扱うかはシステム側の判断に委ねる。