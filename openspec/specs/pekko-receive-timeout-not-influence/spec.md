# pekko-receive-timeout-not-influence Specification

## Purpose
Pekko の `NotInfluenceReceiveTimeout` マーカー契約 (`Actor.scala:165` + `dungeon/ReceiveTimeout.scala:40-42`) を fraktor-rs に導入し、marker 付きメッセージがユーザー配送成功時に receive timeout を誤ってリセットしないことを kernel レベルで保証する。併せて、Pekko で `NotInfluenceReceiveTimeout` を mix-in する `Identify` (`Actor.scala:81`) の内部送出経路も marker を伝える封筒で送るよう統一し、selection 探索がターゲット actor の receive timeout を副作用的にリセットする挙動を排除する。

## Requirements
### Requirement: NotInfluenceReceiveTimeout マーカー trait を介した receive timeout リセット抑制

kernel の actor message 配送経路は、`NotInfluenceReceiveTimeout` マーカー trait を実装する payload を封筒化した `AnyMessage` (以下「not-influence 封筒」) を処理した場合、`ActorContext::reschedule_receive_timeout` を呼んではならない (MUST NOT)。それ以外の user message (control / 通常) の処理成功後は、従来通り reschedule を行わなければならない (MUST)。本契約は Pekko `Actor.scala:165` の `NotInfluenceReceiveTimeout` trait と `dungeon/ReceiveTimeout.scala:40-42` の `!message.isInstanceOf[NotInfluenceReceiveTimeout]` 判定と意味論的に等価でなければならない。

#### Scenario: NotInfluenceReceiveTimeout 実装型を not_influence 経路で送ると timeout が reset されない

- **GIVEN** `NonInfluencingTick` 型が `impl NotInfluenceReceiveTimeout for NonInfluencingTick {}` を持つ
- **AND** actor が `ActorContext::set_receive_timeout(Duration::from_millis(20), _)` で timeout を設定済み
- **WHEN** `AnyMessage::not_influence::<NonInfluencingTick>(NonInfluencingTick)` で封筒化して actor に tell する
- **AND** `invoke` が成功完了する
- **THEN** `reschedule_receive_timeout` は呼ばれない
- **AND** 既存の schedule は維持され、`ReceiveTimeoutState::schedule_generation` は invoke 前後で変化しない

#### Scenario: 通常の AnyMessage::new で送られたメッセージは従来通り reschedule する

- **GIVEN** 同じ `NonInfluencingTick` 型 (marker 実装あり)
- **AND** actor が `ActorContext::set_receive_timeout(_, _)` で timeout 設定済み
- **WHEN** `AnyMessage::new::<NonInfluencingTick>(NonInfluencingTick)` で封筒化 (not_influence = false) して tell する
- **AND** `invoke` が成功完了する
- **THEN** `reschedule_receive_timeout` が呼ばれ、`ReceiveTimeoutState::schedule_generation` が 1 加算される (cancel + schedule が 1 回走った)
- **AND** 挙動は本 change 前と完全に同一 (回帰なし)

#### Scenario: marker trait を実装していない型は AnyMessage::not_influence で封筒化できない

- **GIVEN** `RegularMsg` 型が `NotInfluenceReceiveTimeout` を実装していない
- **WHEN** 開発者が `AnyMessage::not_influence::<RegularMsg>(RegularMsg)` を書く
- **THEN** Rust コンパイラは `T: NotInfluenceReceiveTimeout` の trait bound 違反で **コンパイルエラー** を出さなければならない

### Requirement: `AnyMessage::not_influence` コンストラクタの API と clone 伝播

`AnyMessage` は `not_influence_receive_timeout: bool` field を保持し、以下を満たさなければならない (MUST):

- `AnyMessage::new::<T>(payload)` は `not_influence_receive_timeout = false` で構築する (既存挙動維持、非破壊)。
- `AnyMessage::control::<T>(payload)` は `not_influence_receive_timeout = false` で構築する (既存挙動維持、非破壊)。
- `AnyMessage::not_influence::<T: NotInfluenceReceiveTimeout + Any + Send + Sync + 'static>(payload)` は `not_influence_receive_timeout = true` で構築する。
- `AnyMessage::is_not_influence_receive_timeout(&self) -> bool` は格納した flag を返す公開 getter を持つ。
- `Clone` 実装は `not_influence_receive_timeout` flag を新しいインスタンスに **そのまま** 伝播させる。
- `AnyMessageView::not_influence_receive_timeout(&self) -> bool` を同様に公開する。

#### Scenario: AnyMessage::not_influence が flag を立てる

- **GIVEN** `NonInfluencingTick` が marker を実装
- **WHEN** `let msg = AnyMessage::not_influence(NonInfluencingTick);`
- **THEN** `msg.is_not_influence_receive_timeout() == true`
- **AND** `msg.is_control() == false`
- **AND** `msg.as_view().not_influence_receive_timeout() == true`

#### Scenario: AnyMessage::new は flag を立てない

- **WHEN** `let msg = AnyMessage::new(NonInfluencingTick);`
- **THEN** `msg.is_not_influence_receive_timeout() == false`

#### Scenario: Clone は flag を保持する

- **GIVEN** `msg = AnyMessage::not_influence(NonInfluencingTick)` を clone する
- **WHEN** `let cloned = msg.clone();`
- **THEN** `cloned.is_not_influence_receive_timeout() == true`
- **AND** `msg.is_not_influence_receive_timeout() == true` (clone 元にも影響なし)

### Requirement: `Identify` メッセージの内部封筒化は not-influence 経路を使う

fraktor-rs の kernel は `Identify` メッセージを内部で封筒化する際、`AnyMessage::not_influence::<Identify>(...)` 経由で送らなければならない (MUST)。これは Pekko `Actor.scala:81` の `Identify extends AutoReceivedMessage with NotInfluenceReceiveTimeout` に準拠する。`Identify` 構造体自身も `impl NotInfluenceReceiveTimeout for Identify {}` を宣言し、trait bound を静的に満たさなければならない。

#### Scenario: ActorSelection 経由で送られた Identify は not_influence フラグ付き

- **GIVEN** `ActorSelection::ask_identify` 相当の経路で内部的に Identify が封筒化される
- **WHEN** `modules/actor-core/src/core/kernel/actor/actor_selection/selection.rs` 内の Identify 封筒化箇所が呼ばれる
- **THEN** 封筒化された `AnyMessage` の `is_not_influence_receive_timeout()` は `true` を返す

(受信側で `reschedule_receive_timeout` がスキップされる挙動は後続の「`ActorCellInvoker::invoke` の reschedule ガード」Requirement でカバーする)

#### Scenario: Identify に対する ActorIdentity 返信は not-influence 扱いにしない

- **GIVEN** actor が Identify を受信して応答する (`actor_cell.rs:1513-1522`)
- **WHEN** `sender.try_tell(AnyMessage::new(identity))` で `ActorIdentity` を封筒化して返信する
- **THEN** 返信の `AnyMessage` は `is_not_influence_receive_timeout() == false`
- **AND** ActorIdentity 自体は `NotInfluenceReceiveTimeout` を実装しない (Pekko 側の `ActorIdentity` も non-marker)

### Requirement: `ActorCellInvoker::invoke` の reschedule ガード

`modules/actor-core/src/core/kernel/actor/actor_cell.rs` の `ActorCellInvoker::invoke` における user message 処理成功ブランチは、`message.is_not_influence_receive_timeout() == true` の場合に `ctx.reschedule_receive_timeout()` を呼んではならない (MUST NOT)。`false` の場合は従来通り呼ばなければならない (MUST)。

#### Scenario: not_influence フラグ true のメッセージは reschedule を呼ばない

- **GIVEN** `AnyMessage::not_influence(NonInfluencingTick)` の message と、timeout 設定済みの actor
- **AND** `NonInfluencingTick` の receive 処理が `Ok(())` で終わる
- **WHEN** `ActorCellInvoker::invoke(message)` が呼ばれる
- **THEN** `invoke_user` 成功後に `ctx.reschedule_receive_timeout()` は呼ばれない
- **AND** `ReceiveTimeoutState::schedule_generation` は invoke 前後で変化しない (既存 schedule 維持)

#### Scenario: not_influence フラグ false のメッセージは reschedule を呼ぶ

- **GIVEN** `AnyMessage::new(NonInfluencingTick)` (marker 実装型だが `new` 経由のため flag = false) の message と、timeout 設定済みの actor
- **WHEN** `ActorCellInvoker::invoke(message)` が `Ok(())` で終わる
- **THEN** `ctx.reschedule_receive_timeout()` が **1 回** 呼ばれる
- **AND** `ReceiveTimeoutState::schedule_generation` が invoke 前後で 1 だけ加算される (cancel + schedule の 1 回分)

#### Scenario: 失敗時 (Err) は従来通り reschedule を呼ばず failure report する

- **GIVEN** `AnyMessage::not_influence(NonInfluencingTick)` / `AnyMessage::new(NonInfluencingTick)` のいずれでも、`invoke_user` が `Err(ActorError)` を返す
- **THEN** `ctx.reschedule_receive_timeout()` は呼ばれず、代わりに `cell.report_failure(...)` が呼ばれる
- **AND** この挙動は本 change 前後で不変 (既存 `user_message_failure_does_not_reschedule_receive_timeout` テストが引き続き pass)
