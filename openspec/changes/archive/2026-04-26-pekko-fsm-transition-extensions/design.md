## Context

### 既存 `FsmTransition` の DSL (コード調査結果)

`modules/actor-core/src/core/kernel/actor/fsm/fsm_transition.rs` の public API は 5 メソッドのみ:

- `stay()`, `goto(next_state)`, `stop(reason)`, `unhandled()`, `using(data)`
- 内部 4 フィールド: `next_state: Option<State>`, `next_data: Option<Data>`, `stop_reason: Option<FsmReason>`, `handled: bool`
- `pub(crate) fn into_parts(self) -> (Option<State>, Option<Data>, Option<FsmReason>)` が `Fsm::apply_transition` から呼ばれる唯一のデコンストラクタ

Pekko `FSM.scala:281-307` (`State[S, D]`) の `forMax` / `replying` に対応する DSL が欠落している。

### 既存 `Fsm::handle` / `apply_transition` フロー

`machine.rs:163-264` (`Fsm::handle`, `apply_transition`):

1. `handle` が state handler を呼び出して `FsmTransition` を受け取る
2. `handled == false` なら `unhandled_handler` にフォールバック → まだ false なら no-op 終了
3. `apply_transition` が `into_parts()` で分解、`stop_reason` があれば終了処理
4. explicit transition の場合 `reschedule_state_timeout_for_state(ctx, &next_state)` で state_timeouts から timeout を再設定
5. state / data を更新し、**explicit transition の場合のみ** `transition_observers` を発火 (stay では observer 非発火)

### 既存 state timeout の仕組み

`machine.rs:278-298`:

- フィールド: `state_timeouts: HashMap<State, Duration>`, `timeout_generation: u64`, `timer_key: String` (`fraktor-fsm-timeout-<N>`)
- `ctx.timers().start_single_timer(timer_key, FsmStateTimeout { state, generation }, timeout)` で発火
- `is_stale_timeout` で generation と state の double-check を行い、stale メッセージを discard

### `ctx.timers()` の既存 API

`ClassicTimerScheduler` (`modules/actor-core/src/core/kernel/actor/classic_timer_scheduler.rs`) は既に以下を提供:

- `start_single_timer(key, msg, delay)`, `start_timer_with_fixed_delay`, `start_timer_at_fixed_rate`
- `is_timer_active(key)`, `cancel(key)`, `cancel_all()`

つまり「名前付き timer そのもの」は存在するが、Pekko `FSM` 契約が要求する **"同名再登録時に既にキューに入った late-arrival メッセージが届かないことを保証"** は行わない。本 change の FS-M2 は、この保証を FSM 側で generation token を被せて実現する。

### `ctx.reply()` / `ctx.sender()`

`ActorContext::reply` (`actor_context.rs:201`) は `sender` を持つ envelope 処理中なら `try_tell` で返信し、sender が無ければ `SendError::NoRecipient` を返す。`reply` 自体は dead letter を記録しないため、本 change の `replying` dispatch は `SendError` を `SystemStateShared::record_send_error` で観測可能に記録する。

### AnyMessage の Clone

`AnyMessage: Clone` (`any_message.rs:170`) — 複数 replies を `Vec<AnyMessage>` として保持してもコスト面の問題は既存と同等。

## Goals / Non-Goals

**Goals:**

- Pekko `FSM.scala:281-307` の `forMax` / `replying` と意味論的に等価な `FsmTransition` メソッドを提供する
- Pekko `FSM.scala:579-623` の名前付き arbitrary timer (`startSingleTimer` / `startTimerAtFixedRate` / `startTimerWithFixedDelay` / `cancelTimer` / `isTimerActive`) を提供する
- 既存 FSM コード (`for_max` / `replying` / 名前付き timer を呼ばないもの) の動作は完全に不変
- 同名 timer 再登録時に既存 late-arrival メッセージが state handler に届かないことを保証する (Pekko "no race" 契約)

**Non-Goals:**

- typed FSM (`behavior::Behaviors::withTimers`) の拡張 — typed 側は既に相当機能あり、classic FSM 限定
- Pekko `StateTimeoutAbove` 等の cascade 動作厳密再現 — `for_max` は「遷移ごと install、次の遷移で cancel」の単純動作のみ
- `startTimerAtFixedRate` の precise fixed-rate 保証 — 既存 `ctx.timers()` / scheduler の挙動に従う (drift 許容)
- `FsmTimerFired` を state handler の引数型として expose する — 型自体は `fsm::FsmTimerFired` として pub 公開するが、`Fsm::handle` が先頭で intercept して `payload` を unwrap し state handler には wrap 前のメッセージを渡す (ユーザコードは通常 wrapper を意識しない)
- remote 依存 (gap-analysis AC-M4b) — 別 change で対応

## Decisions

### Decision 1: `FsmTransition` に `for_max_timeout: Option<Option<Duration>>` と `replies: Vec<AnyMessage>` を追加する

- **選択**: 既存 4 フィールド (`next_state`, `next_data`, `stop_reason`, `handled`) に加え、以下を追加する (`handled` は meta flag で `for_max` / `replying` と直交):
  - `for_max_timeout: Option<Option<Duration>>` — 外側 `Option` が「`for_max` 呼び出しの有無」、内側 `Option` が「cancel (`None`) / install (`Some(duration)`)」を示す
  - `replies: Vec<AnyMessage>` — 複数 `replying` 呼び出しを順序保持
- **Rationale**:
  - Pekko `State[S, D]` も `timeout: Option[FiniteDuration]` (+ `setStateTimeout` との競合フラグ) と `replies: List[Any]` で同等の二段管理
  - 外側 `Option` を省略して `Option<Duration>` 単体にすると、"呼び出していない" と "cancel 指示 (`None`)" が区別できず `state_timeouts` との優先順位が曖昧になる
  - `Vec<AnyMessage>` の allocation コストは "`replying` 未呼び出し時は空 Vec (ゼロ heap)" で現行実装と等価 (空 `Vec::new()` は heap 割当てなし)。`FsmTransition` 自身は `into_parts` で move されるため `FsmTransition: Clone` 要件は発生せず、`AnyMessage: Clone` は dispatch 時の個別 reply 処理に使うのみ
- **代替**:
  - (a) `for_max` / `replying` 用に専用 struct を切り出す案 → Pekko は全フィールドを `State` に同居させており、1file1type 原則の例外基準 (判定フロー3 の a/b/c/d) を満たすためそのまま同居
  - (b) `replies` を `Option<AnyMessage>` に制限して複数返信を拒否する案 → Pekko は複数 `replying` を許容 (`State.scala` 実装) するため互換性を崩す

### Decision 2: `for_max` は `state_timeouts[next_state]` より優先する (上書き or cancel)

- **選択**: `apply_transition` 内で **explicit_transition / stay のいずれの経路でも** 以下の優先順位で timeout を評価する (Pekko `forMax` は State に紐づき transition の種類に依らないため):
  1. `transition.for_max_timeout` が `Some(Some(d))` → `cancel_state_timeout(ctx)` → **`self.timeout_generation = self.timeout_generation.wrapping_add(1)` で bump** → `AnyMessage::new(FsmStateTimeout::new(next_state.clone(), self.timeout_generation))` を `ctx.timers().start_single_timer(self.timer_key.clone(), msg, d)` で起動
  2. `transition.for_max_timeout` が `Some(None)` → `cancel_state_timeout(ctx)` + `timeout_generation` bump (bump しないと cancel 直前に enqueue 済の古い timeout メッセージが `is_stale_timeout` で false 判定され state handler に誤配信される)
  3. `transition.for_max_timeout` が `None`:
     - explicit_transition の場合 → 既存の `reschedule_state_timeout_for_state(ctx, &next_state)` をそのまま呼ぶ (内部で bump 済)
     - stay の場合 → 既存 state の timer を変更せず保持 (既存挙動を維持)
- **Rationale**:
  - Pekko `FSM.scala:281-307` の `forMax` は "この遷移に限定した override" で、`state_timeouts` の恒久登録は変更しない
  - 既存 `reschedule_state_timeout_for_state` が `timeout_generation` を bump する契約を持つため、`for_max` 経路でも同等の bump を必須とし、古い `FsmStateTimeout` メッセージを `is_stale_timeout` で確実に弾く
  - `Some(None)` 経路も bump が必要 (scheduler の key 上書きだけでは既に enqueue 済のメッセージを消せない)
- **代替**: `state_timeouts` そのものを遷移時に一時的に書き換える案 → multi-threaded でない (`Fsm` は `&mut self` 持ち) とはいえ、副作用が残るため却下

### Decision 3: `replying` の実行タイミングは "transition_observers 後、次の state handler 起動前"

- **選択**: `apply_transition` 内の順序を以下に固定:
  1. `into_parts()` で分解 (replies / for_max_timeout も取り出す)
  2. `stop_reason` 分岐 → **replies を先に送る** → termination observers (Pekko 同順)
  3. explicit transition → `transition_observers` 発火 → **replies を送る** → `for_max_timeout` 評価 (Decision 2 の 4 分岐のうち explicit 側、`None` の場合は `reschedule_state_timeout_for_state` 呼び出し)
  4. non-explicit (stay) → **replies を送る** → `for_max_timeout` 評価 (Decision 2 の 4 分岐のうち stay 側、`None` の場合は既存 timer 保持で変更なし)
- **Rationale**:
  - Pekko `FSM.scala` の `processEvent` は `notifies` (= state change notification) 後に replies を dispatch
  - `ctx.reply` は `sender` に対する `try_tell` であり同期呼び出しではないが、fraktor-rs の現行 mailbox は送信順序保存 (FIFO) なので Pekko 同等順序を再現できる
  - sender 不在時は `ctx.reply` が `SendError::NoRecipient` を返すため、FSM 側で `ctx.system().state().record_send_error(None, &error)` 相当を呼び、dead-letter 観測経路に `MissingRecipient` として残す
- **代替**: transition observers より前に replies を送る案 → Pekko と順序が逆転し、observer が "このメッセージが返信済か" を前提にした実装を破る

### Decision 4: 名前付き timer は `Fsm` 側で `HashMap<String, FsmNamedTimer>` + generation token を管理する

- **選択**: `Fsm` に以下のフィールドを追加:
  ```rust
  named_timers: HashMap<String, FsmNamedTimer, RandomState>,
  named_timer_generation: u64, // 既存 timeout_generation と同じく単純 u64 (Fsm は &mut self でシーケンシャルアクセス)
  ```
  `FsmNamedTimer { generation: u64, is_repeating: bool, timer_key: String }` を内部型として持つ。各 timer 発火時に actor mailbox へ届くメッセージは `FsmTimerFired { name: String, generation: u64, payload: AnyMessage }` で wrap する。
- **Rationale**:
  - Pekko `FSM.scala:579-623` は内部で `timers: Map[String, Timer]` を持ち、各 `Timer` に `generation: Int` を振って late-arrival を `mapRef` 経由で discard
  - fraktor-rs の `ctx.timers()` (ClassicTimerScheduler) は key 単位で置換するが、既にキューに入ってしまった古い envelope を後から discard できない。FSM 側で generation token を被せるのが最小変更で確実
  - `named_timer_generation` は FSM インスタンスごとに独立で良い (cross-actor 共有不要)
- **代替**:
  - (a) `ClassicTimerScheduler` 側に generation 機能を追加する案 → 影響範囲が FSM 以外にも広がる。timer 再登録時の discard 保証は FSM 固有仕様なので scheduler に漏らす必要はない
  - (b) `ctx.timers()` の `cancel` + 新 `start_*` の順に実行して「古い envelope は自然に届かない」と仮定する案 → cancel より前にメッセージが enqueue 済の場合、並行 mailbox 経路で届く可能性があるため NG

### Decision 5: timer key space を既存 state timeout と分離する

- **選択**: `Fsm::new` で既存の `self.timer_key = "fraktor-fsm-timeout-<N>"` (`N` は `FSM_TIMER_KEY_COUNTER` 採番) に加えて、同じ `N` を共有する named timer 用 prefix `self.named_timer_key_prefix = "fraktor-fsm-named-<N>"` を追加フィールドとして保持する。名前付き timer の scheduler key は `format!("{}-{}", self.named_timer_key_prefix, name)` で生成する (= `fraktor-fsm-named-<N>-<name>`)。
- **Rationale**:
  - `N` を state timeout と named timer で共有することで、同一 FSM インスタンスの timer が prefix で束ねられる (将来の debug / metrics で helpful)
  - infix を `-timeout-` / `-named-` で分けることで scheduler 側の同一 key space 上でも衝突しない
  - ユーザ提供 `name` には prefix を被せるため、ユーザが偶然 `fraktor-fsm-` で始まる name を指定しても FSM インスタンスを越えた衝突は起きない
- **代替**:
  - (a) `FSM_TIMER_KEY_COUNTER` とは別に `FSM_NAMED_TIMER_KEY_COUNTER` を用意する案 → 同一 FSM インスタンスで勝手に別番号になり debug 時に紐付けが面倒
  - (b) `ctx.timers()` の key 空間を直接共有して cancel 時に prefix match する案 → scheduler 側に FSM 知識を漏らすため却下

### Decision 6: `FsmTimerFired` と `FsmNamedTimer` は独立ファイル (1file1type 原則)

- **選択**: 以下のファイル構成:
  - `fsm/fsm_timer_fired.rs` — `pub struct FsmTimerFired { name: String, generation: u64, payload: AnyMessage }`
  - `fsm/fsm_named_timer.rs` — `pub(crate) struct FsmNamedTimer { generation: u64, is_repeating: bool, timer_key: String }` (non-public だが独立)
- **Rationale**:
  - `type-organization.md` の判定フロー ステップ 2 に該当 (テスト対象となる型 → 常に独立ファイル)
  - 既存 `fsm/fsm_state_timeout.rs` / `fsm/fsm_reason.rs` と同パターン
- **代替**: `machine.rs` 内にプライベート定義として書く案 → `FsmTimerFired` は public (ユーザがペイロード unwrap 経路を間接的に観察する可能性) なので独立が妥当。`FsmNamedTimer` は pub(crate) だが `machine.rs` が既に 313 行あり追加で肥大化させない方針で独立化

### Decision 7: `FsmTimerFired` は `Fsm::handle` の先頭で intercept し、payload を clone して生メッセージに差し替える

- **選択**: `Fsm::handle` 先頭で `is_stale_timeout` の前に以下を追加:
  1. `message.downcast_ref::<FsmTimerFired>()` を試す
  2. マッチした場合、`named_timers.get(&fired.name)` の generation と比較
  3. 一致 → `let payload_msg: AnyMessage = fired.payload().clone();` で所有権を取り出し、次いで `named_timers` 更新 (single-shot なら `remove`, repeating ならそのまま) を行ってから `let view = payload_msg.as_view();` で新しい view を作り、**`is_stale_timeout` チェックは skip** して以降のフロー (handlers.get_mut → unhandled_handler fallback → apply_transition) を同関数内で continuation する
  4. 不一致 → 無言で discard (`Ok(())` 早期 return, Pekko 同等)
- **Rationale**:
  - `fired.payload()` は入力 `message: &AnyMessageView` の生存期間に紐づくため、`&mut self` を経由する `named_timers.remove` や `handle` 再入と borrow が衝突する。`AnyMessage` を `clone()` (内部は `Arc` ベースで cheap) することで所有権を `Fsm::handle` のスタックに移し、以降の `&mut self` 借用と競合させない
  - `is_stale_timeout` は `FsmStateTimeout` を検出する分岐で、名前付き timer の payload には State timeout が混入し得ないため intercept 経路では不要 (かつ payload が偶然 `FsmStateTimeout` 型だった場合に stale 誤判定のリスクがある)
  - `self.handle(ctx, &new_view)` として再入する案はスタック深さと borrow 解析の複雑化を招くため避け、同関数内 continuation を採用
  - ユーザ視点では "名前付き timer 発火で通常メッセージが届く" 挙動。`FsmTimerFired` という wrapper を意識させない
- **代替**:
  - (a) `FsmTimerFired` をそのまま state handler に渡してユーザに unwrap させる案 → Pekko API と非互換 + ergonomics 劣化
  - (b) `self.handle(ctx, &inner_view)` 再入方式 → borrow 調停に `fired` の lifetime 延長が必要で複雑化
  - (c) `is_stale_timeout` を skip せず通す案 → payload に `FsmStateTimeout` を意図的に包むユーザはいないはずだが、silently discard するエッジケースを残すリスクがあり却下

### Decision 8: single-timer と repeating-timer の stale 処理の違い

- **選択**:
  - single-shot timer: 発火し generation が一致したら `named_timers.remove(&name)` でエントリを削除 (1回きり)
  - repeating timer (`start_timer_at_fixed_rate` / `start_timer_with_fixed_delay`): エントリを残し、明示的な `cancel_timer(name)` または同名再登録で deactivate
- **Rationale**: Pekko `FSM.scala:600-623` も `Timer.repeat` フラグで同じ挙動。`is_timer_active(name)` は `named_timers.contains_key(name)` で判定できるため single-shot 発火後は `false` を返し Pekko と一致

### Decision 9: FSM 停止時の named timer cleanup 順序

- **選択**: `apply_transition` の `stop_reason` 分岐で、以下の順序を取る:
  1. `cancel_state_timeout(ctx)` で既存 state timeout を止める
  2. `self.data` / `self.terminated` / `self.last_stop_reason` を更新
  3. **replies を dispatch** (Decision 3 の stop_reason 分岐)
  4. `termination_observers` を発火 (observer 内で `is_timer_active` / `cancel_timer` を読む可能性があるため named_timers は **まだ保持**)
  5. `self.named_timers.drain()` で全エントリを取り出し、各 `ctx.timers().cancel(&entry.timer_key)` を呼ぶ。個別 cancel が `Err(SchedulerError)` を返しても最終結果は `Ok(())` 扱いとし、`ctx.system().emit_log(LogLevel::Warn, ...)` で観測可能にする。別のエラーに干渉させない
- **Rationale**:
  - Pekko も observer → cancel の順を取り、observer 発火時点で timer は active、cancel はその直後という契約になっている
  - scheduler の cancel 失敗は FSM 停止契約に影響しない (既に mailbox から抜かれているケースがほとんど)
- **代替**: observer の前に cancel する案 → observer からの `is_timer_active` 呼び出しが突然 false を返し、事後観測として意味を失うため却下

### Decision 10: `for_max(Some(d))` で `d.is_zero()` の扱い

- **選択**: `d.is_zero()` の場合は **`for_max(None)` 相当 (cancel)** として扱う。`FsmTransition::for_max` メソッド内部で `let normalized = if matches!(timeout, Some(d) if d.is_zero()) { None } else { timeout }; self.for_max_timeout = Some(normalized); self` として正規化する (helper 新設は不要、YAGNI)。
- **Rationale**:
  - Pekko `FSM.scala` の `forMax(Duration.Zero)` は "no timeout" 扱い (`FiniteDuration` からの `setTimer` 分岐で Zero は install されない)
  - 既存 `set_state_timeout` は `assert!(!timeout.is_zero(), ...)` で panic させているが、`for_max` はユーザの runtime 値 (計算結果) を受ける可能性が高く、panic にすると "0 を一瞬でも通したら落ちる" ugly semantics になるため cancel 化で許容
- **代替**:
  - (a) `set_state_timeout` と同じ panic → Pekko と非互換かつ実用性低い
  - (b) `Some(Duration::ZERO)` を install → scheduler 側で即発火してしまい意味をなさない

## Risks / Trade-offs

### Risk 1: `FsmTimerFired` 型名の視認性

- **影響**: `FsmTimerFired` 自体は `fsm::FsmTimerFired` として pub export されるため、ユーザが `use fraktor_actor_core_rs::fsm::FsmTimerFired;` で import し downcast 試行するコードを書くことは技術的に可能。ただし:
  - field は全て非 pub、`new` / accessor は `pub(crate)` のため、**外部からの struct literal 構築・インスタンス生成は言語レベルで不可能** (Decision 6 + Phase 4.1)
  - payload 経路は Decision 7 に従い `Fsm::handle` で unwrap されて state handler には届かないため、user-visible なインスタンスは通常存在しない
  - 残存リスクは "型名そのものが namespace 上に見える" ことだけ
- **緩和**: rustdoc に "exported for trait-bound propagation only; not intended for direct user use" を明記。実質的な衝突リスクは pub(crate) 採用により解消済

### Risk 2: 名前付き timer の generation counter 枯渇

- **影響**: `named_timer_generation: u64` は `u64::MAX` まで増加可能で実用上枯渇しない (Decision 4 で Atomic ではなく単純 u64 を採用、`&mut self` シーケンシャルアクセス前提)。仮に wrapping_add で 0 に戻った場合、運悪く同 name で `generation = 0` 同士が並ぶと古い envelope が誤って通る
- **緩和**: `wrapping_add(1)` で 0 を skip する実装 (`if next == 0 { next = 1 }`) を採用。既存 `timeout_generation` と同じ方針

### Risk 3: `for_max` と `set_state_timeout` の相互作用で次々の遷移で意図せず cascade

- **影響**: ユーザが `goto(S).for_max(Some(5s))` で S に入り、次の遷移で `goto(S).for_max(None)` すると `state_timeouts[S]` が恒久登録されていても cancel される。これは Pekko と同じだが、初見では混乱しうる
- **緩和**: rustdoc で `for_max(None)` が "transient cancel であり `state_timeouts` は変更しない" を明記。テストで "for_max(None) 後の次の遷移で state_timeouts が復活すること" を検証

### Risk 4: `replying` のメッセージ順序と transition observers 順序

- **影響**: Pekko は observer → replies 順。本実装もそれに従うが、observer 内で `ctx.reply` を直接呼ぶユーザコードがあると replies が二重発火になり得る
- **緩和**: rustdoc で "observer 内での `ctx.reply` と `replying` の併用は避ける" を明記。テストで標準パスの順序のみ検証 (多重発火は検出対象外)

### Risk 5: `FsmTimerFired` intercept 時の Clone コスト

- **影響**: Decision 7 に従い intercept 経路で `fired.payload().clone()` を明示的に行う。`AnyMessage` 自体は `Arc` ベースの shared 構造のため clone は参照カウント増加のみで cheap だが、wrap / unwrap に 1 層のオーバーヘッドは残る
- **緩和**: `FsmTimerFired::payload` を `AnyMessage` として保持することで clone が `Arc::clone` に集約される。fire 頻度の高い repeating timer でも wrap 層 1 段 + Arc bump のみ (実測不要と判断)

### Risk 6: 5 新メソッド追加による `Fsm` 型の API 膨張

- **影響**: `Fsm` は既に `when` / `when_unhandled` / `set_state_timeout` / `on_transition` / `on_termination` / `initialize` / `handle` / `state_name` / `state_data` / `is_terminated` / `generation` / `last_stop_reason` の 12 public method。本 change で `start_single_timer` / `start_timer_at_fixed_rate` / `start_timer_with_fixed_delay` / `cancel_timer` / `is_timer_active` の 5 件を追加し合計 17 public method (`for_max` / `replying` は `FsmTransition` 側なので別カウント)
- **緩和**: Pekko `FSM` trait も同等規模 (約 20 method) で参照実装として許容範囲。関心があるユーザは typed FSM か `LoggingFsm` wrapper を使える。ドキュメントに "high-level DSL が必要なら LoggingFsm 検討" を追記

### Risk 7: `FsmTimerFired` wrapper により既存 stash / unstash パスと非互換

- **影響**: `ctx.stash()` した envelope の中に `FsmTimerFired` が紛れていた場合、unstash 後に FSM が受け取ると既に generation が古く silently discard される
- **緩和**: Pekko も同じ挙動 (stash 中 timer が "late-arrival" 扱いになる) なので仕様として許容。rustdoc で "名前付き timer 発火中に stash していると unstash 後に discard される可能性" を明記
