## Why

Pekko `Actor.scala:165` の `NotInfluenceReceiveTimeout` マーカー契約は、「このメッセージ型は receive timeout タイマーをリセットしない」ことを保証する (Pekko `Actor.scala:81` で `Identify` が mix-in、`dungeon/TimerSchedulerImpl.scala:37-40` の timer 内部メッセージにも適用)。fraktor-rs は現在 `actor_cell.rs:1527` の user message 処理成功ブランチで **無条件に** `reschedule_receive_timeout()` を呼ぶため、ユーザーが「このメッセージは timeout に影響させない」と宣言する手段が存在せず、periodic / 内部由来のメッセージが idle 検知を壊す Pekko 非互換状態にある。

gap-analysis (`docs/gap-analysis/actor-gap-analysis.md` AC 系 medium 一覧) の AC-M5 として長期間積み残されており、fraktor-rs 側には「NotInfluenceReceiveTimeout 契約を満たすメッセージが timeout を reset しない」ことを pin するテストも存在しない。

## What Changes

- **NEW** `pub trait NotInfluenceReceiveTimeout: Any + Send + Sync` を `modules/actor-core/src/core/kernel/actor/messaging/` 配下に導入。
- `AnyMessage` に `not_influence_receive_timeout: bool` フラグを追加し、`AnyMessage::not_influence<T: NotInfluenceReceiveTimeout + ...>(...)` コンストラクタでフラグを立てる経路を用意する (既存 `AnyMessage::new` / `::control` は非破壊)。
- **破壊的変更**: `AnyMessage::from_parts` / `AnyMessage::into_parts` / `AnyMessage::from_erased` の tuple 要素数を 3 → 4 に拡張 (`not_influence_receive_timeout: bool` を末尾に追加)。CLAUDE.md「後方互換不要」方針で許容。
- (additive) `AnyMessageView::with_control` は 3 引数で維持し、`AnyMessageView::with_flags(payload, sender, is_control, not_influence_receive_timeout)` を新規追加。`AnyMessage::as_view` は新コンストラクタを呼ぶよう内部書き換え (視点として API 追加のみで破壊なし)。
- フレームワーク内蔵型 `Identify` (既存) に `impl NotInfluenceReceiveTimeout for Identify {}` を付与し、内部封筒化経路 (`actor_selection/selection.rs:77`) を `AnyMessage::not_influence` 経由に書き換える。**`ReceiveTimeout` struct には marker を付けない** (Pekko `Actor.scala:154` vs `Actor.scala:165` に準拠、design Open Questions 参照)。
- `ActorCellInvoker::invoke` (`actor_cell.rs:1527`) の user message 成功ブランチで `if !message.is_not_influence_receive_timeout() { ctx.reschedule_receive_timeout(); }` のガードを追加 (Pekko `dungeon/ReceiveTimeout.scala:40-42` 準拠、出口側のみ)。
- `invoke` 入口の cancel-before-receive は fraktor-rs には存在せず (design Decision 3 確認済)、本 change では出口側ガードのみを導入する。
- 契約を pin する **新規テスト 5 件** を `actor_cell/tests.rs` (一部は `any_message/tests.rs`) に追加し、加えて既存 1 件の regression を維持する (計 6 シナリオで契約を pin):
  - `not_influence_message_skips_reschedule` (marker 付きでフラグ立て → reschedule されない)
  - `regular_message_reschedules_receive_timeout` (marker 無し → 従来通り reschedule、回帰)
  - `identify_message_is_not_influence_by_internal_path` (internal Identify 経路の flag 検証)
  - `not_influence_flag_is_preserved_on_clone` (Clone で flag 保持)
  - `view_exposes_not_influence_flag` (`AnyMessageView` 側の公開 getter)
  - `user_message_failure_does_not_reschedule_receive_timeout` (**既存**、本 change 後も引き続き pass を維持)
- `docs/gap-analysis/actor-gap-analysis.md` の AC-M5 行を done 化、medium カウントを 11 → 10 に更新。

## Capabilities

### New Capabilities
- `pekko-receive-timeout-not-influence`: `NotInfluenceReceiveTimeout` マーカーによる receive timeout リセット抑制の契約 (Pekko `Actor.scala:165` の trait + `dungeon/ReceiveTimeout.scala:40-42` の出口側判定に対応。入口側 `:71-76` は fraktor-rs に該当経路がなく本 change 対象外、design Decision 3 参照) と、`Identify` 内部封筒化経路および `AnyMessage::not_influence` 経由のユーザー利用経路における非リセット保証を定義する。

### Modified Capabilities

なし。既存 spec に receive timeout の要件レベル契約は散在しておらず、本 change が新規 capability (`pekko-receive-timeout-not-influence`) として AC-M5 専用契約を立てる方針。

## Impact

- **kernel**: `modules/actor-core/src/core/kernel/actor/messaging/any_message.rs` (新フィールド + `not_influence` / `is_not_influence_receive_timeout` / `Clone`・`Debug` の対応、`from_parts`/`into_parts`/`from_erased` の tuple 4 要素化)、`modules/actor-core/src/core/kernel/actor/messaging/any_message_view.rs` (flag field + `with_flags` + `not_influence_receive_timeout` getter)、`modules/actor-core/src/core/kernel/actor/messaging/` 配下に `not_influence_receive_timeout.rs` 新設、`modules/actor-core/src/core/kernel/actor/messaging/identify.rs` に `impl NotInfluenceReceiveTimeout for Identify`、`modules/actor-core/src/core/kernel/actor/actor_cell.rs:1527` (invoke 成功ブランチにガード)。
- **internal callers**: `Identify` を封筒化する経路 (`actor_selection/selection.rs:77`) を `AnyMessage::not_influence` 経由に書き換え。`AnyMessage::from_parts` / `into_parts` / `from_erased` を呼ぶ全 caller を grep で全特定し tuple 4 要素化に追従させる。
- **public API (additive)**: `NotInfluenceReceiveTimeout` trait と `AnyMessage::not_influence` コンストラクタ、`AnyMessage::is_not_influence_receive_timeout`、`AnyMessageView::not_influence_receive_timeout`、`AnyMessageView::with_flags` が新規公開 API として追加。`AnyMessage::new` / `::control` / `AnyMessageView::new` / `AnyMessageView::with_control` の signature は不変。
- **public API (BREAKING)**: `AnyMessage::from_parts` / `AnyMessage::into_parts` / `AnyMessage::from_erased` の tuple 要素数が 3 → 4 に拡張される (`not_influence_receive_timeout: bool` 末尾追加)。
- **docs**: `docs/gap-analysis/actor-gap-analysis.md` の AC-M5 行を done 化、版番号 (第 13 版) と medium カウント (11 → 10) を更新。
- **tests**: `actor_cell/tests.rs` / `any_message/tests.rs` に **新規 5 件** 追加 (reschedule 有/無、Identify internal path、Clone 伝播、View getter)、既存 `user_message_failure_does_not_reschedule_receive_timeout` の regression 維持。既存テスト全 pass。
- **スコープ非対象**: `ClassicTimerScheduler` (`classic_timer_scheduler.rs`) の timer メッセージへの `NotInfluenceReceiveTimeout` 自動付与は AC-M5 の延長線上ではあるが、別 change として切り出す (要件として spec には含めず、本 change では touch しない)。
