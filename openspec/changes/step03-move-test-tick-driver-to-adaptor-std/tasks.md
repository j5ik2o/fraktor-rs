## 1. 事前調査と環境確認

- [x] 1.1 `Grep` で `TestTickDriver` 全言及を再確認（起案時: 51 ファイル・215 箇所、workspace 内）
- [x] 1.2 `Grep` で旧 API の全言及を確認: `ActorSystem::new_empty\|ActorSystem::new_empty_with` を対象に caller 件数を集計（`new_empty` は関連関数であり instance method 形式は存在しないため `::` 参照のみで網羅できる。新名 `new_empty_actor_system*` は本 change で新設するため事前調査では対象外）
- [x] 1.3 `actor-core` の内部で `TestTickDriver` / `new_empty*` を使うインラインテストを棚卸し（`src/**/tests.rs`）
- [x] 1.4 `actor-adaptor-std/src/std/tick_driver/` の既存 `StdTickDriver` / `TokioTickDriver` の構造を再確認（配置パターン、feature gate の書き方を写す）
- [x] 1.5 `actor-core` 側で `actor-adaptor-std` から見える必要のある内部 API を列挙（design Decision 2 の「実装詳細」参照）:
  - `ActorSystem::state` field の可視性（private なら直接アクセス不可）
  - `SystemStateShared::mark_root_started` メソッドの可視性
  - `SystemState::build_from_owned_config` の可視性
  - 上記を直接 `pub` 化する案 vs. `ActorSystem::new_started_from_config(config)` のような公開 constructor を追加する案を比較し、最小差分を選択（field 直接公開は避ける方向）
- [x] 1.6 Cargo dev-cycle の挙動を確認（`actor-core` の `[dev-dependencies]` に `actor-adaptor-std` を試験追加して `cargo metadata` が通ること）

## 2. actor-adaptor-std 側に TestTickDriver 移設

- [x] 2.1 `modules/actor-adaptor-std/src/std/tick_driver/test_tick_driver.rs` 新規作成。`actor-core` 側の `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/test_tick_driver.rs` の内容を移植
- [x] 2.2 移植後、`use super::{...};` 等の相対 import を `actor-adaptor-std` の module 構造に合わせて `use fraktor_actor_core_rs::core::kernel::actor::scheduler::tick_driver::{SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverKind, TickDriverProvision, TickDriverStopper, TickFeedHandle, next_tick_driver_id};` のような crate 境界 import に書き換え
- [x] 2.3 `modules/actor-adaptor-std/src/std/tick_driver.rs` に `#[cfg(feature = "test-support")] mod test_tick_driver;` と `#[cfg(feature = "test-support")] pub use test_tick_driver::TestTickDriver;` を追加（alphabetical 位置を保つ）
- [x] 2.4 `cargo build -p fraktor-actor-adaptor-std-rs --features test-support` で単体ビルド成功確認

## 3. actor-adaptor-std 側に new_empty_actor_system 系の自由関数を新設

- [x] 3.1 **（先行）** actor-core 側で必要な internal API を公開: tasks 1.5 の調査結果に基づき、選択した戦略（案 A: field + method を pub 化 / 案 B: `ActorSystem::new_started_from_config` など公開 constructor 追加）を先に実装。この段階では `new_empty*` 削除前なので、既存機能を壊さずに公開点を増やすだけ
- [x] 3.2 actor-core の `cargo check -p fraktor-actor-core-rs --lib` が通ることを確認（3.1 の変更後）
- [x] 3.3 `modules/actor-adaptor-std/src/std/system/` ディレクトリ新規作成
- [x] 3.4 `modules/actor-adaptor-std/src/std/system/empty_system.rs` 新規作成。design Decision 2 のサンプルコードをベースに `new_empty_actor_system` / `new_empty_actor_system_with<F>` を実装する。3.1 で公開した API（pub 化 field / method または公開 constructor）を使用。必要な型を `fraktor_actor_core_rs::...::{ActorSystem, ActorSystemConfig, SystemState, SystemStateShared}` 等から import
- [x] 3.5 `modules/actor-adaptor-std/src/std/system.rs` 新規作成。module file として `#[cfg(feature = "test-support")] mod empty_system;` と `#[cfg(feature = "test-support")] pub use empty_system::{new_empty_actor_system, new_empty_actor_system_with};` を記述
- [x] 3.6 `modules/actor-adaptor-std/src/std.rs` に `#[cfg(feature = "test-support")] pub mod system;` を追加
- [x] 3.7 `cargo build -p fraktor-actor-adaptor-std-rs --features test-support` で新規自由関数を含むビルド成功確認

## 4. actor-core 側から TestTickDriver と new_empty* を削除

- [x] 4.1 `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/test_tick_driver.rs` ファイル削除
- [x] 4.2 `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver.rs` から `#[cfg(any(test, feature = "test-support"))] mod test_tick_driver;` と `#[cfg(any(test, feature = "test-support"))] pub use test_tick_driver::TestTickDriver;` を削除
- [x] 4.3 `modules/actor-core/src/core/kernel/system/base.rs` の line 65-97 付近の以下 **2 method のみ** を削除（他の `#[cfg(any(test, feature = "test-support"))]` 要素があれば触らない）:
  - `#[cfg(any(test, feature = "test-support"))] pub fn new_empty() -> Self { ... }` （docstring + `#[must_use]` 属性含めて）
  - `#[cfg(any(test, feature = "test-support"))] pub fn new_empty_with<F>(configure: F) -> Self where F: FnOnce(ActorSystemConfig) -> ActorSystemConfig { ... }` （同上）
  削除後、`base.rs` 内に他の `#[cfg(any(test, feature = "test-support"))]` ゲートがあればそのまま残す（本 change の対象外）
- [x] 4.4 削除後、lib prod ビルドのみ中間確認: `cargo check -p fraktor-actor-core-rs --no-default-features --lib` と `cargo check -p fraktor-actor-core-rs --features test-support --lib` が通ることを確認。test 系（`--tests` / `--all-targets`）は Phase 5〜6 で import 更新するまで失敗する想定のため、本タスクでは実行しない

## 5. actor-core の Cargo.toml に dev-dependency 追加

- [x] 5.1 `modules/actor-core/Cargo.toml` の `[dev-dependencies]` に以下を追加:
  ```toml
  fraktor-actor-adaptor-std-rs = { workspace = true, features = ["test-support"] }
  ```
  既存 dev-deps と alphabetical 順序を保つ位置に挿入
- [x] 5.2 `cargo metadata -p fraktor-actor-core-rs` で循環依存エラーが出ないことを確認

## 6. actor-core のインラインテスト・統合テストを書き換え

- [x] 6.1 インラインテスト（`modules/actor-core/src/**/tests.rs`）の `use crate::core::kernel::actor::scheduler::tick_driver::TestTickDriver;` 形式を `use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;` に一括置換（起案時: 20+ ファイル）
- [x] 6.2 `base.rs` インラインテスト等で `ActorSystem::new_empty()` / `ActorSystem::new_empty_with(...)` を使っている箇所を `fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system()` / `new_empty_actor_system_with(...)` に書き換え（import 追加）
- [x] 6.3 統合テスト（`modules/actor-core/tests/*.rs`）の `use fraktor_actor_core_rs::...::TestTickDriver;` 形式を同様に書き換え（起案時: 8 ファイル）
- [x] 6.4 `cargo test -p fraktor-actor-core-rs --lib` でインラインテスト成功確認
- [x] 6.5 `cargo test -p fraktor-actor-core-rs --features test-support` で統合テスト成功確認

## 7. 下流クレートの import path 更新

- [x] 7.1 `modules/cluster-core/` 配下のテストで `TestTickDriver` / `new_empty*` 参照を書き換え
- [x] 7.2 `modules/stream-core/` 配下のテストで同上
- [x] 7.3 `modules/stream-adaptor-std/` 配下のテストで同上
- [x] 7.4 `modules/persistence-core/` 配下のテストで同上
- [x] 7.5 `modules/actor-adaptor-std/tests/*.rs` で同上
- [x] 7.6 `showcases/std/` 配下の example で同上
- [x] 7.7 各 crate を個別コマンドで確認:
  - `cargo test -p fraktor-cluster-core-rs`
  - `cargo test -p fraktor-stream-core-rs`
  - `cargo test -p fraktor-stream-adaptor-std-rs`
  - `cargo test -p fraktor-persistence-core-rs`
  - `cargo test -p fraktor-actor-adaptor-std-rs --features test-support`
  - `cargo test -p fraktor-showcases-std --features advanced`（advanced example がある場合）

## 8. 最終確認 Grep

- [x] 8.1 `Grep "use .*fraktor_actor_core_rs.*TestTickDriver"` で 0 hits 確認
- [x] 8.2 `Grep "use crate::core::.*::TestTickDriver"` で `actor-core` 配下の残存が無いこと確認（`actor-adaptor-std` 側の新 path は `use fraktor_actor_core_rs::...::TickDriver` trait 参照なので除外）
- [x] 8.3 `Grep "ActorSystem::new_empty"` で caller 残存が無いこと確認
- [x] 8.4 `Grep "::TestTickDriver"` で唯一の定義元が `actor-adaptor-std/src/std/tick_driver/test_tick_driver.rs` であることを確認

## 9. ビルド・テスト・clippy 検証

- [x] 9.1 `cargo build --workspace --no-default-features` で workspace 全体の default-features 無効ビルドが通過
- [x] 9.2 `cargo build --workspace --all-features` で workspace 全体ビルド成功
- [x] 9.3 `cargo test --workspace` で全テスト pass
- [x] 9.4 `cargo clippy --workspace --all-targets -- -D warnings` で clippy clean（lib レベルで少なくとも）

## 10. spec 整合確認

- [x] 10.1 `openspec validate step03-move-test-tick-driver-to-adaptor-std --strict` で artifact 整合確認
- [x] 10.2 新規 capability `actor-test-driver-placement` の Scenario が本 change 後の実態と一致していることを目視確認

## 11. 全体 CI 確認

- [x] 11.1 `./scripts/ci-check.sh ai all` で workspace 全体確認（CLAUDE.md ルールに従い完了を待つ）
- [x] 11.2 失敗があれば原因を特定し、修正してから再実行
- [x] 11.3 すべて green になったら、コミット・PR 作成の前にユーザー確認を取る

## 12. ドキュメント更新

- [x] 12.1 `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 1「責務 B（ダウンストリーム統合テスト用 API 公開）」の進捗記述を更新（B-1 `TestTickDriver` / `new_empty*` 移設済み、B-2 残り: mock / probe / その他ヘルパ → step04）
- [x] 12.2 hand-off メモに「actor-core の `[dev-dependencies]` に `actor-adaptor-std` を追加した（dev-cycle）」点を記録
- [x] 12.3 step04 の proposal に「`new_empty*` は step03 で移設済みのため step04 では対象外」と追記（proposal は step03 archive 前までに更新）
