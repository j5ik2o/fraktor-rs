## Why

`actor-core/test-support` feature 配下には、step03 完了時点で以下が残っている:

- **責務 B-2 残**: テスト fixture として `pub` 化されているが、実際の caller は actor-core 内部の inline test のみのもの
  - `ActorRef::new_with_builtin_lock` (caller: actor-core inline tests のみ)
  - `SchedulerRunner::manual` (caller: actor-core inline tests のみ)
  - `state::booting_state` / `state::running_state` モジュール宣言 (caller: actor-core inline tests のみ)
- **責務 C 全体**: 内部 API の `pub(crate)` → `pub` 格上げ
  - `Behavior::handle_message` / `handle_start` / `handle_signal`
  - `TypedActorContext::from_untyped`
  - `TickDriverBootstrap` struct + `provision` メソッド
  - その他 `#[cfg(any(test, feature = "test-support"))]` ゲートで `pub` 化されている内部 API

> **step04 統合**: step04 で計画していた「専用テストヘルパ crate `fraktor-actor-test-rs` 切り出し」は、調査の結果 **移設対象が実質不在** (proposal で想定された `MockActorRef` / `TestProbe` は存在せず、上記 B-2 残はすべて actor-core 内部 inline test のみが caller) と判明したため close された。step04 の対象だった B-2 残は本 change (step05) に統合する。
>
> **step03 からの dev-cycle 知見**: actor-core の inline test は同一クレート二バージョン問題により外部 crate 由来のヘルパを使えない。本 change では「外部公開しないなら `pub(crate)` 化、必要なら inline test を統合 test に移行して外部 crate 経由」の二択を case-by-case で判断する。

これらを残した状態では「本体 feature flag 経由で内部 API を露出する」アンチパターンが温存される。本 change はその全数解消を行う。

本 change は Strategy B の第 5 ステップ（責務 B-2 残 + 責務 C 統合処理）。step06（feature 削除）の地ならしになる。

## What Changes

- `actor-core` 配下で `#[cfg(any(test, feature = "test-support"))]` により可視性を拡大している全 14 箇所を棚卸し（Grep 全数収集）
- 各箇所について以下のいずれかの戦略を割り当て（design 段階で決定）:
  - **A 案 (pub(crate) 化)**: 外部 caller がない場合、`pub(crate)` (or `#[cfg(test)]`) に戻す。inline test だけが caller のものはこちらが既定
  - **B 案 (正式 public API 化)**: 外部 caller があり、internal とは言えない場合、正規 public API として docs / 型シグネチャを整備
  - **C 案 (inline test を統合 test 移行)**: actor-core 内部 inline test が caller の場合、テストを `tests/*.rs` に移し、外部 caller と同じ経路 (actor-adaptor-std や直接 internal を呼ばない再設計) を取る
- 割り当てに従いリファクタ:
  - A: 可視性戻し + cfg gate 削除
  - B: 正規 public API として整備 (docs、型整理)
  - C: inline test 移行 + caller 修正
- `actor-core/src/` から `#[cfg(any(test, feature = "test-support"))]` 経由で可視性拡大している箇所が **0 件** になるのを目標にする（純粋な `#[cfg(test)]` は保持）
- **BREAKING（workspace-internal）**: 一部 API のパス・シグネチャ・可視性が変わる（ダウンストリームテストおよび actor-core 自身の inline test の修正が必要）

**Non-Goals**:
- `test-support` feature 自体の削除は step06 で行う（本 change 完了後は feature が空 `[]` または限りなく空に近づく想定）
- `actor-core` の public API surface の再設計（責務 C 解消で露出する必要のある API のみ整備）
- `actor-adaptor-std::TestTickDriver` / `new_empty_actor_system*` の再配置（step03 で確定済み、本 change のスコープ外）
- step03 で導入した actor-core 内部の `pub(crate)` 限定 dev-cycle workaround helper (`tick_driver/tests/test_tick_driver.rs` の TestTickDriver、`base/tests.rs` / `typed/system/tests.rs` の `new_empty*`) の撤去（これらは inline test 移行を前提とする別 change で扱う）

## Capabilities

### New Capabilities
- なし

### Modified Capabilities
- `actor-test-driver-placement`: step03 で確立した「test-support feature 経由で内部 API を露出しない」原則を、TestTickDriver / new_empty\* に限らず actor-core 全要素に拡張する Scenario を追加（MODIFIED）

OpenSpec validation 要件を満たすため、design / specs フェーズで delta を設計する。候補:
- 案 A: 新規 capability `actor-core-api-visibility-governance` を ADDED し、「feature flag 経由で内部 API の可視性を拡大してはならない」一般則を明文化
- 案 B: 既存 `actor-test-driver-placement` の MODIFIED として一般則の Scenario を追加（重複を避ける観点ではこちらが筋）

## Impact

- **Affected code**:
  - `modules/actor-core/src/**` の各所（`#[cfg(any(test, feature = "test-support"))]` 削除 + 可視性戻し / 移動）:
    - `core/typed/behavior.rs` (handle_message / handle_start / handle_signal)
    - `core/typed/actor/actor_context.rs` (from_untyped)
    - `core/kernel/system/state.rs` (booting_state / running_state mod declarations)
    - `core/kernel/system/state/system_state.rs` (register_guardian_pid)
    - `core/kernel/system/state/system_state_shared.rs` (register_guardian_pid)
    - `core/kernel/actor/actor_ref/base.rs` (new_with_builtin_lock)
    - `core/kernel/actor/scheduler/scheduler_runner.rs` (manual)
    - `core/kernel/actor/scheduler/tick_driver/bootstrap.rs` (TickDriverBootstrap struct + provision)
    - `core/kernel/actor/scheduler/tick_driver.rs` (TickDriverBootstrap re-export)
  - 戦略 C 採用箇所では actor-core inline test の移動 (`src/**/tests.rs` → `tests/*.rs`)
  - ダウンストリームの統合テスト (戦略 B 採用箇所で API パスが変わった場合)
- **Affected APIs**: workspace-internal な API シグネチャ・可視性変更
- **Affected dependencies**: なし
- **Release impact**: pre-release phase につき外部影響は軽微
