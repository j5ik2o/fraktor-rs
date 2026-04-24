## Why

gap-analysis 第20版時点で actor-core に残る medium ギャップ 3 件のうち、remote 依存 (AC-M4b) を除いた **2 件は classic FSM の transition DSL / timer 管理不足** であり、いずれも独立して scope できる:

### FS-M1: `FsmTransition` の `forMax` / `replying` 未実装

Pekko の `State[S, D]` (= fraktor-rs の `FsmTransition<State, Data>`) は以下を提供する (`FSM.scala:281-307`):

- **`forMax(timeout: Duration)`** — 次遷移先に限定した state timeout を上書き (Pekko default の `setStateTimeout(state, ...)` を遷移側から個別 override)。`Duration.Inf` 相当で明示的にキャンセルもできる
- **`replying(replyValue)`** — 遷移確定前 (= `using` / state timeout install より前) に現在の sender へ返信する

fraktor-rs の `FsmTransition` は `stay / goto / stop / unhandled / using` の 5 メソッドのみで、いずれも未対応。結果として:

- 遷移ごとに timeout を変える FSM ユースケース (cancel / retry window 等) が書けず、`set_state_timeout` を state 固定で使わざるをえない
- sender への返信が state handler 戻り値 (`FsmTransition`) 経由でできず、`ctx.sender().tell(...)` を handler 本体で直接書く必要がある (Pekko 互換性のサンプル翻訳が 1:1 で書き写せない)

### FS-M2: 名前付き arbitrary timer 未実装

Pekko の `FSM` trait は state 外で独立した名前付き timer を持つ (`FSM.scala:579-623`):

- **`startSingleTimer(name, msg, delay)`** / **`startTimerAtFixedRate(name, msg, interval)`** / **`startTimerWithFixedDelay(name, msg, delay)`**
- **`cancelTimer(name)`**
- **`isTimerActive(name)`**

既存 timer (`setTimer`) が同名で再登録された場合は **previous timer をキャンセルし、既にキューに入っていたメッセージが届かないことを保証** する (Pekko 原典コメント参照)。

fraktor-rs は state timeout (`set_state_timeout`) と `ctx.timers()` (Pekko typed timers) しか持たず、classic FSM 級の名前付き arbitrary timer が存在しない。よって以下が書けない:

- state に縛られない lifetime timer (login session 時間切れ等)
- 同時に複数の独立 timer を名前で区別して管理する FSM
- "timer が生きているか" の実行時問い合わせ

## What Changes

### FS-M1.1: `FsmTransition::for_max` 追加

```rust
/// Overrides the state timeout for the transition target (Pekko `forMax`).
///
/// `Some(d)` installs a transient timeout for the next state, overriding
/// whatever default `set_state_timeout` registered. `None` explicitly
/// cancels any pre-configured timeout on the target state
/// (Pekko `Duration.Inf` marker).
#[must_use]
pub const fn for_max(mut self, timeout: Option<Duration>) -> Self
```

- `FsmTransition` に `for_max_timeout: Option<Option<Duration>>` を追加 (outer `Option` は "指定なし" / "指定あり"、inner は "cancel" / "Some(duration)")
- `Fsm::handle` / 遷移実行パスで `for_max_timeout` を state_timeouts[next_state] より優先
- tests: handler が `goto(S).for_max(Some(5.seconds))` を返したとき state_timeouts[S] が無視され 5s timeout が設定されること、`for_max(None)` で既存 state_timeouts[S] が一時キャンセルされること

### FS-M1.2: `FsmTransition::replying` 追加

```rust
/// Reply to the sender of the current message (Pekko `replying`).
///
/// Replies are queued on the transition descriptor and dispatched
/// **after** the transition observers fire but **before** the new state
/// handler is installed, matching Pekko's ordering.
#[must_use]
pub fn replying(mut self, reply: AnyMessage) -> Self
```

- `FsmTransition` に `replies: Vec<AnyMessage>` を追加 (複数 `replying` 呼び出しを保持、Pekko と同じ)
- `Fsm::handle` が transition を適用する際、`ctx.reply(reply)` で現在の sender へ replies を順に送信する
- `ctx.reply` が `SendError` を返した場合は `SystemStateShared::record_send_error` 経由で観測可能に記録し、残りの replies の送信を継続する
- tests: `stay().replying(Reply::Ack)` で sender が受信すること、複数 replies が送信順序で届くこと、sender 不在の場合は `SendError::NoRecipient` が dead-letter 観測経路へ記録されること

### FS-M2.1: 名前付き timer API 追加

`Fsm` 本体のメソッドとして以下を提供:

```rust
/// Start a single-shot timer with the given name.
///
/// If a timer with the same name is already active it is cancelled and
/// any enqueued message from it is discarded before the new timer fires.
pub fn start_single_timer(
  &mut self,
  ctx: &mut ActorContext<'_>,
  name: impl Into<String>,
  msg: AnyMessage,
  delay: Duration,
);

/// Start a fixed-rate (`AtFixedRate`) or fixed-delay repeating timer.
pub fn start_timer_at_fixed_rate(&mut self, ctx: &mut ActorContext<'_>, name: impl Into<String>, msg: AnyMessage, interval: Duration);
pub fn start_timer_with_fixed_delay(&mut self, ctx: &mut ActorContext<'_>, name: impl Into<String>, msg: AnyMessage, delay: Duration);

/// Cancel a named timer. No-op if the name does not exist.
pub fn cancel_timer(&mut self, ctx: &mut ActorContext<'_>, name: &str);

/// True iff the named timer is still scheduled (not fired for single-shot,
/// not cancelled for repeating).
pub fn is_timer_active(&self, name: &str) -> bool;
```

- 内部は `HashMap<String, FsmTimerHandle>` (名前 → scheduler ハンドル)
- **同名 timer 再登録時は既存を必ず cancel し**、`FsmTimerHandle` に generation token を付けて既にキューに入った late-arrival メッセージを `handle` 側で discard する (Pekko の "no race" 保証)
- timer 発火メッセージは `FsmTimerFired { name, generation, payload }` 型で actor mailbox に入り、`Fsm::handle` が先頭で見て: (a) active generation と一致すれば `payload` を通常の FSM message として state handler に配信、(b) 不一致なら discard
- timer キャンセル / FSM 停止時は全 timer を clean up

### Gap-analysis 更新

- 実装時点の現行最新版 + 1 として FS-M1 (forMax / replying) + FS-M2 (名前付き timer) を done 化
- 残存 medium を AC-M4b (remote 依存 / deferred) の 1 件に更新

## Capabilities

### Modified Capabilities

- **`classic-fsm-runtime`** (既存、`modules/actor-core/src/core/kernel/actor/fsm/`):
  - `FsmTransition` に `for_max` / `replying` メソッド追加
  - `Fsm` に名前付き timer API (`start_single_timer` / `start_timer_at_fixed_rate` / `start_timer_with_fixed_delay` / `cancel_timer` / `is_timer_active`) 追加
  - 既存 `set_state_timeout` / `stay` / `goto` / `using` 等は不変
  - state timeout と同じ key space を使わないよう内部 timer key 生成を分離 (`fraktor-fsm-timeout-<N>` 既存 + `fraktor-fsm-named-<N>-<name>` 新規)

### New Capabilities

なし (classic FSM の既存 capability 拡張のみ)

## Impact

**影響を受けるコード**:

- `modules/actor-core/src/core/kernel/actor/fsm/fsm_transition.rs`:
  - `for_max_timeout: Option<Option<Duration>>` field 追加
  - `replies: Vec<AnyMessage>` field 追加 (Clone bound か `into_parts` 返却形を再検討 — Data: Clone のままでよいか再確認)
  - `for_max` / `replying` メソッド追加
  - `into_parts` シグネチャ拡張 (`(Option<State>, Option<Data>, Option<FsmReason>, Option<Option<Duration>>, Vec<AnyMessage>)` 相当)
- `modules/actor-core/src/core/kernel/actor/fsm/machine.rs`:
  - `named_timers: HashMap<String, FsmNamedTimer>` (generation token 付き) field 追加
  - `start_single_timer` / `start_timer_at_fixed_rate` / `start_timer_with_fixed_delay` / `cancel_timer` / `is_timer_active` メソッド追加
  - `handle` で遷移実行時に `for_max_timeout` と `replies` を適用するロジック追加
  - `handle` 先頭で `FsmTimerFired` マーカー型を見て generation matching + discard ロジック追加
- `modules/actor-core/src/core/kernel/actor/fsm/fsm_timer_fired.rs` (新規、1file1type 原則):
  - `FsmTimerFired { name, generation, payload }` ペイロード型
- `modules/actor-core/src/core/kernel/actor/fsm/fsm_named_timer.rs` (新規、1file1type 原則):
  - `FsmNamedTimer { generation, is_repeating, timer_key }` 内部状態
- `modules/actor-core/src/core/kernel/actor/fsm/tests.rs`:
  - `for_max` 系 5 ケース (Some / None / stay 経路 / state_timeouts との interaction / `Duration::ZERO` cancel 正規化)
  - `replying` 系 3 ケース (basic / multiple replies / missing-recipient dead-letter observation)
  - 名前付き timer 系 6 ケース (single / fixed-rate / fixed-delay / cancel / is_active / 同名再登録時の late-arrival discard)
- `docs/gap-analysis/actor-gap-analysis.md`:
  - 実装時点の現行最新版 + 1 の entry を追加、FS-M1 / FS-M2 done 化、残存 medium を AC-M4b 1 件に更新

**影響を受ける公開 API 契約**:

- `FsmTransition` 型の public メソッド数が増加 (既存メソッドのシグネチャは不変) — 純粋な追加変更、破壊的変更なし
- `Fsm` 型に新規 public メソッドが追加される (既存メソッドは不変) — 同上
- `FsmTimerFired` 型を `fsm::FsmTimerFired` として型のみ public export する。`new` / accessor は `pub(crate)` で、state handler には unwrap 済み payload が渡るため通常の利用者 API にはしない
- `FsmTransition::into_parts` は `pub(crate)` のため外部ユーザー影響なし

**挙動変更**:

- Pekko 翻訳コードで `goto(next).using(data).forMax(timeout)` / `stay().replying(Reply::Ok)` 相当の Rust コードが直接書けるようになる
- 既存 FSM コード (`for_max` / `replying` を呼ばない) の動作は完全に不変 — field default (`None` / 空 Vec) が no-op

## Non-goals

- **typed FSM の拡張** — typed 層は既に `behavior::Behaviors::withTimers` / typed timers 経由で相当機能を提供済み。classic FSM (untyped) の runtime 不足のみを scope
- **state_timeouts と forMax の cascade 動作の厳密な Pekko 完全再現** — 本 change では "遷移ごとに install、次の遷移で cancel" の単純動作を実装。Pekko の `StateTimeoutAbove` 等のエッジケースは別 change で必要性が出たら対応
- **`startTimerAtFixedRate` の precise fixed-rate 保証** — Pekko も "ベストエフォート" であり、fraktor-rs の scheduler 挙動に従う (timer drift が生じ得る)
- **`FsmTimerFired` のユーザー向け API 化** — 型名は内部 wrapper の trait-bound 伝搬と crate 内テストのため `fsm::FsmTimerFired` として見えるが、構築・accessor は `pub(crate)` のままにする。ユーザー目線では `msg` がそのまま state handler に届く
