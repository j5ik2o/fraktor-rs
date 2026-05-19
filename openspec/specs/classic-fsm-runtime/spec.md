# classic-fsm-runtime Specification

## Purpose
classic FSM runtime に Pekko FSM 相当の遷移拡張（一時 state timeout override、返信 queue、名前付き timer API）を追加し、no_std core 境界と既存モジュール規約を維持する。

## Requirements

### Requirement: `FsmTransition` は Pekko `forMax` 相当の一時 state timeout override を提供する

classic FSM runtime は `FsmTransition::for_max(timeout: Option<Duration>) -> Self` を提供し、遷移ごとの state timeout override を表現できなければならない (MUST)。`Some(duration)` は次に有効になる state timeout を一時的に上書きし、`None` は次に有効になる state timeout を一時的に cancel しなければならない (MUST)。この override は `Fsm::set_state_timeout` の恒久登録を書き換えてはならない (MUST NOT)。

`Duration::ZERO` を受け取った場合は panic ではなく `None` と同じ cancel 指示に正規化しなければならない (MUST)。`for_max` が指定された場合、explicit transition と `stay()` のどちらでも既存 timer を cancel し、timeout generation を進め、既に enqueue 済みの古い `FsmStateTimeout` を stale として discard できなければならない (MUST)。

#### Scenario: `for_max(Some(duration))` は登録済み state timeout より優先される

- **GIVEN** state `S` に `set_state_timeout(S, 30s)` が登録されている
- **WHEN** state handler が `goto(S).for_max(Some(5s))` を返す
- **THEN** FSM は `5s` の一時 timeout を schedule する
- **AND** `set_state_timeout(S, 30s)` の恒久登録は変更されない

#### Scenario: `for_max(None)` は次の timeout だけを cancel する

- **GIVEN** state `S` に `set_state_timeout(S, 30s)` が登録されている
- **WHEN** state handler が `goto(S).for_max(None)` を返す
- **THEN** FSM は既存 state timeout timer を cancel する
- **AND** timeout generation を進め、古い timeout delivery を stale として discard する
- **AND** 後続の通常 `goto(S)` では `set_state_timeout(S, 30s)` が再び適用される

#### Scenario: `stay().for_max(...)` でも一時 timeout override が適用される

- **GIVEN** FSM が state `S` で処理中である
- **WHEN** state handler が `stay().for_max(Some(2s))` を返す
- **THEN** FSM は state を変更せず `2s` の一時 timeout を schedule する

#### Scenario: `Duration::ZERO` は cancel に正規化される

- **WHEN** state handler が `goto(S).for_max(Some(Duration::ZERO))` を返す
- **THEN** FSM は panic しない
- **AND** `for_max(None)` と同じ cancel 経路を実行する

### Requirement: `FsmTransition` は Pekko `replying` 相当の返信 queue を提供する

classic FSM runtime は `FsmTransition::replying(reply: AnyMessage) -> Self` を提供し、state handler の戻り値に sender への返信を積めなければならない (MUST)。複数回呼び出された `replying` は呼び出し順を保持して dispatch されなければならない (MUST)。

返信 dispatch の順序は Pekko `FSM` と同等に、explicit transition では transition observers の後、state timeout の再設定前でなければならない (MUST)。`stay()` では replies の dispatch 後に `for_max` 指定があれば timeout override を評価し、`for_max` 指定がなければ既存 timer を保持しなければならない (MUST)。stop transition では state/data/stop reason の更新後、termination observers の前に replies を dispatch しなければならない (MUST)。

`ActorContext::reply` は sender 不在時に `SendError::NoRecipient` を返すため、FSM は `ctx.reply` の `SendError` を握りつぶしてはならない (MUST NOT)。個別 reply の失敗は `SystemStateShared::record_send_error` で dead-letter 観測経路へ記録し、残りの replies の dispatch を継続しなければならない (MUST)。

#### Scenario: sender が存在する場合、replying は sender へ返信する

- **GIVEN** sender 付き envelope を FSM が処理している
- **WHEN** state handler が `stay().replying(AnyMessage::new(Ack))` を返す
- **THEN** sender は `Ack` を受信する

#### Scenario: 複数 replying は順序を保持する

- **WHEN** state handler が `stay().replying(first).replying(second)` を返す
- **THEN** sender には `first`、`second` の順で dispatch される

#### Scenario: sender 不在時の reply 失敗は dead-letter 観測経路へ記録される

- **GIVEN** sender が存在しない envelope を FSM が処理している
- **WHEN** state handler が `stay().replying(reply)` を返す
- **THEN** `ctx.reply(reply)` は `SendError::NoRecipient` を返す
- **AND** FSM はその失敗を `SystemStateShared::record_send_error(None, &error)` 相当で記録する
- **AND** dead-letter snapshot には `DeadLetterReason::MissingRecipient` の entry が残る

#### Scenario: 1 件の reply 失敗は後続 replies を止めない

- **GIVEN** 複数 replies が transition に積まれている
- **WHEN** 途中の reply dispatch が `SendError` を返す
- **THEN** FSM は失敗を記録する
- **AND** 後続 replies の dispatch を継続する

### Requirement: `Fsm` は Pekko FSM 相当の名前付き timer API を提供する

classic FSM runtime は `Fsm::start_single_timer`、`Fsm::start_timer_at_fixed_rate`、`Fsm::start_timer_with_fixed_delay`、`Fsm::cancel_timer`、`Fsm::is_timer_active` を提供しなければならない (MUST)。これらの timer は FSM インスタンス内の名前で管理され、state timeout 用 timer key と衝突してはならない (MUST NOT)。

同名 timer の再登録では既存 timer を先に cancel し、新しい generation を割り当てなければならない (MUST)。既に mailbox に入った古い timer 発火メッセージは `FsmTimerFired { name, generation, payload }` の generation mismatch により state handler へ渡してはならない (MUST NOT)。single-shot timer は generation が一致して発火した時点で inactive になり、repeating timer は明示 cancel または同名再登録まで active のままでなければならない (MUST)。

#### Scenario: single-shot timer は payload を unwrap して state handler へ渡す

- **GIVEN** `start_single_timer(ctx, "tick", payload, delay)` が成功している
- **WHEN** timer が発火し、generation が active entry と一致する
- **THEN** FSM は `FsmTimerFired` wrapper を state handler へ渡さない
- **AND** state handler は元の `payload` を通常メッセージとして受け取る
- **AND** `"tick"` は inactive になる

#### Scenario: repeating timer は発火後も active のまま残る

- **GIVEN** `start_timer_at_fixed_rate` または `start_timer_with_fixed_delay` で timer が登録されている
- **WHEN** generation が一致する timer 発火を FSM が処理する
- **THEN** payload は state handler へ渡される
- **AND** `is_timer_active(name)` は true を返す

#### Scenario: `cancel_timer` は active timer を停止し inactive にする

- **GIVEN** 名前付き timer が active である
- **WHEN** `cancel_timer(ctx, name)` が呼ばれる
- **THEN** scheduler key は cancel される
- **AND** `is_timer_active(name)` は false を返す

#### Scenario: 同名再登録後の古い timer 発火は discard される

- **GIVEN** timer `"tick"` が generation `1` で登録済みである
- **WHEN** 同じ `"tick"` が再登録され generation `2` になる
- **AND** generation `1` の `FsmTimerFired` が後から mailbox で処理される
- **THEN** FSM はその message を state handler へ渡さず `Ok(())` で discard する

#### Scenario: FSM 停止時は termination observers 後に全 named timers を cleanup する

- **GIVEN** FSM に active な named timers が存在する
- **WHEN** state handler が `stop(reason)` を返す
- **THEN** FSM は state timeout を cancel し、state/data/stop reason を更新する
- **AND** replies を dispatch してから termination observers を発火する
- **AND** termination observers 実行時点では `is_timer_active(name)` で named timer を観測できる
- **AND** termination observers 後に全 named timers を drain し、scheduler timer を cancel する

### Requirement: FSM transition extensions は no_std core と既存モジュール規約を保持する

実装は `modules/actor-core` の no_std 境界を守り、`alloc::*` と既存 core API のみを使用しなければならない (MUST)。`FsmTimerFired` と `FsmNamedTimer` は 1 型 1 ファイルで配置し、`fsm.rs` の mod / `pub use` 配線に追加しなければならない (MUST)。`FsmTimerFired` は wrapper として内部で構築され、state handler には unwrap 済み payload が渡らなければならない (MUST)。

#### Scenario: 新規型は独立ファイルで配線される

- **WHEN** 実装後の `modules/actor-core/src/core/kernel/actor/fsm/` を確認する
- **THEN** `fsm_timer_fired.rs` と `fsm_named_timer.rs` が存在する
- **AND** `fsm.rs` に mod 宣言が追加されている
- **AND** `FsmTimerFired` は `fsm::FsmTimerFired` として型のみ public export される

#### Scenario: no_std core に std 依存を導入しない

- **WHEN** 実装差分を確認する
- **THEN** `modules/actor-core` に `std::` import は追加されない
- **AND** timer/reply 失敗の観測は既存 `record_send_error` または core logging API で行われる
