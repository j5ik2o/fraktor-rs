## Phase 1: 準備と参照確認

- [x] 1.1 Pekko `FSM.scala:281-307` (`State[S, D]` の `forMax` / `replying`) と `FSM.scala:579-623` (名前付き timer) を参照し、挙動契約を確定
- [x] 1.2 既存 `fsm_transition.rs` のフィールド構成 (`next_state` / `next_data` / `stop_reason` / `handled`) と `into_parts` シグネチャを確認
- [x] 1.3 既存 `machine.rs::apply_transition` (L226-264) の stop_reason / explicit_transition / transition_observers 発火順序を確認し、`replying` / `for_max` の挿入ポイント (design Decision 2, 3) を確定
- [x] 1.4 既存 `reschedule_state_timeout_for_state` (L273-284) と `cancel_state_timeout` (L286-288) の挙動、`FSM_TIMER_KEY_COUNTER` と `timer_key` (`fraktor-fsm-timeout-<N>`) の採番方式を確認
- [x] 1.5 `ClassicTimerScheduler::start_single_timer` / `start_timer_with_fixed_delay` / `start_timer_at_fixed_rate` / `is_timer_active` / `cancel` (`classic_timer_scheduler.rs:37-94`) のシグネチャと戻り値型を確認
- [x] 1.6 `ActorContext::reply` (`actor_context.rs:201`) は sender 不在時に `SendError::NoRecipient` を返し、dead-letter 記録は行わないことを確認。`try_tell` の失敗経路と合わせ、FSM 側で `record_send_error` が必要な箇所を確定
- [x] 1.7 `AnyMessage` の trait bounds (`Clone + Send + Sync + 'static`) と `AnyMessage::new<T>(t: T)` / `AnyMessage::as_view()` の既存実装を確認し、`FsmTimerFired` が `Send + Sync + 'static` を自動導出できること (field: `String` / `u64` / `AnyMessage` すべて満たす) と `FsmTimerFired::payload` の包み方・再 view 化経路を確定
- [x] 1.8 `fsm.rs` (mod エントリ) の既存 mod 宣言順を確認し、新 mod 2 件 (`fsm_timer_fired` / `fsm_named_timer`) の alphabetical 挿入位置を決定
- [x] 1.9 `ActorContext::stash` / `unstash_all` 経路で envelope がどう保持されるかを確認し、`FsmTimerFired` wrapper が stash 中に generation 不一致で discard されるリスク (design Risk 7) の実体を検証。必要なら追加テスト / 設計見直し
- [x] 1.10 Pekko `FSM.scala` の `forMax(Duration.Zero)` / `State.forMax` 実装箇所を再読し、`Duration.Zero` 受領時の挙動 (no-op cancel 扱い) を参照ソースで確定 (design Decision 10 の根拠補強)
- [x] 1.11 fraktor-actor-core-rs kernel 内でのエラー / warn レベル観測パターンを確認。`SendError` は `ctx.system().state().record_send_error(target, &error)`、`SchedulerError` など send 以外の best-effort cleanup 失敗は `ctx.system().emit_log(LogLevel::Warn, ...)` で観測する方針を確定し、Phase 3.5 / 5.5 / 6.3 で "swallow + 観測可能" を満たす
- [x] 1.12 `docs/gap-analysis/actor-gap-analysis.md` の現行最新版号と、**AC-M2 の現行状態** (別 change で対応済か未完か) を同時に確認する (前回 MB-M2 done 化が第17版、以降 `pekko-death-watch-duplicate-check` / `pekko-dispatcher-alias-chain` / `pekko-dispatcher-primary-id-alignment` / `pekko-fault-dispatcher-hardening` 等で版号が進んでいる可能性あり)。Phase 9.1 で採用する版号は "現行最新 + 1"、Phase 9.5 の残存 medium list は AC-M2 状態に応じて調整

## Phase 2: `FsmTransition::for_max` の追加

- [x] 2.1 `fsm_transition.rs` に `for_max_timeout: Option<Option<Duration>>` field を追加 (既存 stay/goto/stop/unhandled コンストラクタも `for_max_timeout: None` で初期化)
- [x] 2.2 `FsmTransition::for_max(self, timeout: Option<Duration>) -> Self` メソッド追加 (`#[must_use]`, rustdoc は "Pekko `forMax`", "`None` は state_timeouts を一時 cancel" を明記。Phase 2.6 で追加する `state_timeouts registration is not modified` / `Duration::ZERO is normalized to cancel` もこの rustdoc に統合する)
- [x] 2.3 `into_parts` の戻り値を `(Option<State>, Option<Data>, Option<FsmReason>, Option<Option<Duration>>)` に拡張し、caller (`machine.rs::apply_transition`) を 1 箇所追随
- [x] 2.4 `machine.rs::apply_transition` の既存構造を書き換える (design Decision 2 / 3 に従う):
  - **既存コードの削除**: L250-252 の `if explicit_transition { self.reschedule_state_timeout_for_state(ctx, &next_state)?; }` ブロックを削除 (timeout 設定は observers 発火後に移動するため)
  - **新規挿入ポイント**: state / data 更新後、`transition_observers` 発火後、replies dispatch 後に以下の for_max 優先分岐を追加 (**explicit_transition / stay のいずれでも適用**):
    - `Some(Some(d))` → `cancel_state_timeout(ctx)` → `self.timeout_generation = self.timeout_generation.wrapping_add(1)` (bump) → `AnyMessage::new(FsmStateTimeout::new(next_state.clone(), self.timeout_generation))` を `ctx.timers().start_single_timer(self.timer_key.clone(), msg, d)` で起動
    - `Some(None)` → `cancel_state_timeout(ctx)` + `timeout_generation` bump (cancel 前に enqueue 済の古い FsmStateTimeout を is_stale_timeout で確実に弾くため)
    - `None` + explicit_transition → 既存 `reschedule_state_timeout_for_state(ctx, &next_state)` を呼ぶ (内部で bump 済)
    - `None` + stay → 既存 timer 保持 (何もしない、本 change 以前の挙動を維持)
- [x] 2.5 `FsmTransition::for_max` の内部で `d.is_zero()` の場合は `for_max(None)` 相当 (cancel) に正規化する (design Decision 10)
- [x] 2.6 rustdoc に "for_max is transient; state_timeouts registration is not modified" と "Duration::ZERO is normalized to cancel" を明記

## Phase 3: `FsmTransition::replying` の追加

- [x] 3.1 `fsm_transition.rs` に `replies: Vec<AnyMessage>` field を追加 (各コンストラクタは `Vec::new()` で初期化)
- [x] 3.2 `FsmTransition::replying(self, reply: AnyMessage) -> Self` メソッド追加 (`#[must_use]`, rustdoc は "Pekko `replying`", "複数呼び出しは順序保持" を明記)
- [x] 3.3 `into_parts` の戻り値に `Vec<AnyMessage>` を加え、caller を追随 (`(Option<State>, Option<Data>, Option<FsmReason>, Option<Option<Duration>>, Vec<AnyMessage>)`)
- [x] 3.4 `machine.rs::apply_transition` で replies の dispatch 分岐を追加 (design Decision 3):
  - stop_reason 分岐: state/data 更新 → **replies dispatch** → termination_observers 発火 → named_timers cleanup (Decision 9 step 2→3→4→5 の順)
  - explicit transition: `transition_observers` 発火 → **replies dispatch** → Phase 2.4 の for_max 評価 4 分岐 (`None` + explicit は既存 `reschedule_state_timeout_for_state` 呼び出し)
  - non-explicit (stay): **replies dispatch** → Phase 2.4 の for_max 評価 4 分岐 (`Some(Some(d))` / `Some(None)` なら適用、`None` + stay は既存 timer 保持で変更なし)
- [x] 3.5 replies dispatch ループは `for reply in replies { if let Err(err) = ctx.reply(reply) { ctx.system().state().record_send_error(None, &err); /* continue */ } }` 相当で **個別 `SendError` は観測可能な形で記録しつつ残りの replies を継続** する (Pekko の `sender ! reply` は個別失敗を伝播しない契約のため)。`.agents/rules/ignored-return-values.md` 準拠で "fire-and-forget + 観測可能" を満たすため、`let _ = ...;` での無言 swallow は禁止
- [x] 3.6 sender 不在時は `ctx.reply` が `SendError::NoRecipient` を返すため、3.5 の `record_send_error(None, &err)` 経路で `DeadLetterReason::MissingRecipient` として観測されることを保証

## Phase 4: `FsmTimerFired` と `FsmNamedTimer` 型の新設

- [x] 4.1 `fsm/fsm_timer_fired.rs` を新規作成:
  - `#[derive(Clone)] pub struct FsmTimerFired { name: String, generation: u64, payload: AnyMessage }` (`AnyMessage::new(FsmTimerFired)` で wrap するため `Clone + Send + Sync + 'static` 必須。field 3 つの auto trait から `Send + Sync` は自動導出される想定、`'static` は String/u64/AnyMessage すべて満たす)
  - `pub(crate) fn new(name: String, generation: u64, payload: AnyMessage) -> Self` (構築は fraktor-rs 内部の `Fsm::start_*_timer` のみ)
  - `pub(crate) fn name(&self) -> &str`, `pub(crate) fn generation(&self) -> u64`, `pub(crate) fn payload(&self) -> &AnyMessage` (accessor は intercept 経路でのみ使用)
  - rustdoc に "exported for trait-bound propagation but intended to be transparent to state handlers; direct construction or observation by user code is strongly discouraged" を明記 (型自体は pub export されるが user は通常 wrapper を意識しない)
- [x] 4.2 `fsm/fsm_named_timer.rs` を新規作成:
  - `pub(crate) struct FsmNamedTimer { generation: u64, is_repeating: bool, timer_key: String }`
  - `pub(crate) const fn new(generation: u64, is_repeating: bool, timer_key: String) -> Self`
  - accessor は必要最小限 (`generation()`, `is_repeating()`, `timer_key()`)
- [x] 4.3 `fsm/fsm_timer_fired/tests.rs` 新規作成 (fraktor-rs 規約: `fsm/fsm_timer_fired.rs` と `fsm/fsm_timer_fired/tests.rs` を並置する `foo.rs` + `foo/tests.rs` パターン、既存 archive `bounded_deque_message_queue.rs` + `bounded_deque_message_queue/tests.rs` と同形。`pub(crate)` 可視性なので同 crate 内テストで `use super::*` によりアクセス可能、最低限の new / accessor 検証 3 件)
- [x] 4.4 `fsm.rs` (mod エントリ) に 2 新 mod 宣言と `pub use fsm_timer_fired::FsmTimerFired;` 追加 (既存 `pub use` のアルファベット順に挿入)

## Phase 5: `Fsm` に名前付き timer API を追加

- [x] 5.1 `Fsm` struct に以下の 3 field を追加:
  - `named_timers: HashMap<String, FsmNamedTimer, RandomState>`
  - `named_timer_generation: u64`
  - `named_timer_key_prefix: String` (`fraktor-fsm-named-<N>`, 既存 `timer_key` と同じ `N` を共有、design Decision 5)
- [x] 5.2 `Fsm::new` 初期化で `named_timers: HashMap::with_hasher(RandomState::new())`, `named_timer_generation: 0`, `named_timer_key_prefix: format!("fraktor-fsm-named-{}", N)` を追加 (N は既存 `timer_key` 生成時に採番した `FSM_TIMER_KEY_COUNTER` の値を再利用するため、`timer_key` 採番と同じ fetch_add 結果を local 変数で保持して両 prefix に使う)
- [x] 5.3 internal helper `next_named_timer_generation(&mut self) -> u64` を追加 (wrapping_add(1) で 0 を skip、design Risk 2)。**CQS 違反**: `&mut self` + 戻り値だが、bump と read を分離すると「read → 別の bump → 元の caller が古い値を使う」race の表面化を招くため Vec::pop 相当の "読み取り兼状態進行" pattern として許容 (`.agents/rules/rust/cqs-principle.md` の許容例 "Vec::pop 相当")。doc comment は `// CQS exception: bump and read are inseparable in a single call, same pattern as Vec::pop` (Rust の `std::sync::atomic` を想起させる "atomic" は使わない)
- [x] 5.4 internal helper `named_timer_key(&self, name: &str) -> String` を追加 (`format!("{}-{}", self.named_timer_key_prefix, name)`, design Decision 5)
- [x] 5.5 `Fsm::start_single_timer(&mut self, ctx: &mut ActorContext<'_>, name: impl Into<String>, msg: AnyMessage, delay: Duration) -> Result<(), ActorError>` 追加:
  - `let name: String = name.into();` で所有権を確保
  - 既存同名 timer がある場合: **先に `self.named_timers.remove(&name)` で owned `FsmNamedTimer` を取り出し** (借用を直ちに解放)、その上で `ctx.timers().cancel(&old.timer_key)` を呼ぶ。**cancel の `Err` は `ctx.system().emit_log(LogLevel::Warn, ...)` で記録して swallow** — Pekko は previous timer 停止失敗を伝播せず新 timer install を継続する契約
  - 新 generation を採番 → `FsmTimerFired::new(name.clone(), generation, msg)` を `AnyMessage::new(...)` で包んで `ctx.timers().start_single_timer(timer_key.clone(), fired, delay)` を呼ぶ。`SchedulerError` は既存 `Fsm::scheduler_error_to_actor_error` helper で `ActorError` に変換して伝播
  - `self.named_timers.insert(name, FsmNamedTimer::new(generation, false, timer_key))`
- [x] 5.6 `Fsm::start_timer_at_fixed_rate` / `start_timer_with_fixed_delay` を 5.5 と同構造で追加 (違いは `ctx.timers()` の呼び先と `FsmNamedTimer::new(.., is_repeating: true, ..)`)。変換は `scheduler_error_to_actor_error` を共有
- [x] 5.7 `Fsm::cancel_timer(&mut self, ctx: &ActorContext<'_>, name: &str) -> Result<(), ActorError>` 追加:
  - `named_timers.remove(name)` があれば `ctx.timers().cancel(&entry.timer_key)` を呼び、`SchedulerError` は `scheduler_error_to_actor_error` で変換して伝播
  - 無ければ no-op (Pekko 同等、戻り値は `Ok(())`)
- [x] 5.8 `Fsm::is_timer_active(&self, name: &str) -> bool` 追加 (`named_timers.contains_key(name)`)
- [x] 5.9 rustdoc で同名再登録時の "previous timer cancellation + late-arrival discard guarantee" を明記

## Phase 6: `Fsm::handle` での `FsmTimerFired` intercept 統合

- [x] 6.1 `Fsm::handle` 先頭 (`is_stale_timeout` より前) に intercept 分岐を追加 (design Decision 7):
  - `message.downcast_ref::<FsmTimerFired>()` で試す
  - マッチ → `named_timers.get(&fired.name)` と generation 比較
    - 一致 → `let payload_msg: AnyMessage = fired.payload().clone();` で所有権確保 (`AnyMessage` の clone は `Arc` ベースで cheap)
    - `is_repeating == false` なら `named_timers.remove(&fired.name)`
    - `let payload_view = payload_msg.as_view();` で新 view を作り、**`is_stale_timeout` チェックは skip** して以降のフロー (handlers.get_mut → unhandled_handler fallback → apply_transition) を同関数内で continuation
    - 不一致 → `Ok(())` で早期 return (discard)
- [x] 6.2 借用解析の検証 (`fired` の参照解放 → `named_timers` の `&mut` 借用 → `payload_msg.as_view()` の `&` 借用を順序立てて実施)。`self.handle` 再入は避ける (design Decision 7 代替案 b)
- [x] 6.3 FSM 停止時の cleanup 順序を固定 (design Decision 9):
  1. `cancel_state_timeout(ctx)`
  2. `self.data` / `self.terminated` / `self.last_stop_reason` 更新
  3. replies dispatch
  4. `termination_observers` 発火 (observer 内で `is_timer_active` / `cancel_timer` が読める状態を維持)
  5. `self.named_timers.drain()` で全エントリを取り出し `ctx.timers().cancel(&entry.timer_key)` を順に呼ぶ。個別 cancel の `Err` は **`ctx.system().emit_log(LogLevel::Warn, ...)` で記録**して主処理を中断させない (FSM 停止契約に影響させない)

## Phase 7: テスト追加

- [x] 7.1 `fsm/tests.rs` に `for_max` 系 5 ケース追加:
  - `for_max_some_installs_transient_timeout` — `goto(S).for_max(Some(5s))` が state_timeouts[S] を override
  - `for_max_none_cancels_state_timeout` — `for_max(None)` で cancel、次の explicit transition で state_timeouts が復活
  - `stay_applies_for_max_override` — `stay().for_max(Some(2s))` で stay 経路でも for_max 指定の timeout が install されることを確認 (Decision 2 の stay + Some(Some(d)) 分岐)
  - `state_timeouts_reapplies_after_for_max_override` — 遷移 A: `for_max(Some(5s))` install → 遷移 B: 通常 `goto(S)` で `state_timeouts[S]` が再び有効になり 5s ではなく本来の値で発火
  - `for_max_zero_duration_normalized_to_cancel` — `for_max(Some(Duration::ZERO))` で panic せず cancel 動作 (design Decision 10)
- [x] 7.2 `fsm/tests.rs` に `replying` 系 3 ケース追加:
  - `replying_basic_delivers_to_sender`
  - `replying_multiple_preserves_order`
  - `replying_without_sender_records_missing_recipient_dead_letter` (sender None のとき `SendError::NoRecipient` が `record_send_error` 経由で `DeadLetterReason::MissingRecipient` として記録されることを検証)
- [x] 7.3 `fsm/tests.rs` に 名前付き timer 系 6 ケース追加:
  - `start_single_timer_fires_and_unwraps_payload`
  - `start_timer_at_fixed_rate_fires_repeatedly`
  - `start_timer_with_fixed_delay_fires_repeatedly`
  - `cancel_timer_prevents_fire`
  - `is_timer_active_tracks_lifecycle` (single 発火後 false / repeating は cancel まで true)
  - `restart_same_name_discards_late_arrival` — 直接 `AnyMessage::new(FsmTimerFired::new(name, old_gen, payload))` を構築して `Fsm::handle` に渡すことで「旧 generation envelope が mailbox にいる状態」を再現。`Fsm::start_single_timer(name, ..., ...)` を先に呼んで generation を進めた後、手動で旧 envelope を handle に渡し、state handler に届かず silently discard されることを検証 (test では state handler 内で bool flag を立てて "呼ばれなかった" を assert)
- [x] 7.4 `fsm/tests.rs` に stop-cleanup テスト追加:
  - `stop_cancels_all_named_timers` — `FsmReason::Normal` で停止後、`named_timers.is_empty()` かつ scheduler 側の timer もすべて cancel
- [x] 7.5 `fsm/fsm_timer_fired/tests.rs` に new / accessor 3 ケース追加

## Phase 8: CI / lint 検証

- [x] 8.1 `rtk cargo test -p fraktor-actor-core-rs --lib` で全テスト pass 確認 (新規 tests 18 件: for_max 5 + replying 3 + 名前付き timer 6 + stop-cleanup 1 + FsmTimerFired 3 + 既存 regression)
- [x] 8.2 `rtk cargo test -p fraktor-actor-core-rs --tests` でインテグレーション pass 確認
- [x] 8.3 `./scripts/ci-check.sh ai dylint` で新規ファイルの dylint ゼロ確認 (特に type-per-file / mod-file / module-wiring / use-placement / rustdoc / tests-location / ambiguous-suffix / cfg-std-forbid)
- [x] 8.4 `./scripts/ci-check.sh ai all` を実行し exit 0 を確認
- [x] 8.5 clippy / rustdoc / type-per-file lint で新規警告ゼロ確認 (`ai all` 内で統合)

## Phase 9: gap-analysis 更新

- [x] 9.1 `docs/gap-analysis/actor-gap-analysis.md` に新版 entry を追加 (Phase 1.12 で確認した現行最新版号 + 1 を採用):
  - サマリーテーブルに `第<N+1>版、FS-M1 + FS-M2 完了反映後` を追加し、残存 medium 数を更新
- [x] 9.2 FS-M1 行 (`forMax` / `replying`) を done 化:
  - `✅ **完了 (change `pekko-fsm-transition-extensions`)** —` プレフィックス
  - 実装参照を `fsm_transition.rs` (`for_max` / `replying` メソッド) に書換
  - 最終列を `~~medium~~ done` に
- [x] 9.3 FS-M2 行 (名前付き timer) を done 化:
  - 実装参照を `machine.rs` (`start_single_timer` / `start_timer_at_fixed_rate` / `start_timer_with_fixed_delay` / `cancel_timer` / `is_timer_active`) と `fsm_timer_fired.rs` / `fsm_named_timer.rs` に書換
  - 最終列を `~~medium~~ done` に
- [x] 9.4 Phase A3 / 該当章の「完了済み」リストに FS-M1 + FS-M2 を追加
- [x] 9.5 Phase A3 / 該当章の「残存 medium」を `AC-M4b (deferred)` 1 件に更新 (AC-M2 が既に別 change で対応済なら除外、未完なら残す)

## Phase 10: PR 発行とレビュー対応

- [x] 10.1 branch `impl/pekko-fsm-transition-extensions` を切って PR 発行、base は main
- [x] 10.2 PR 本文に以下を含める:
  - Pekko `FSM.scala:281-307` (forMax / replying) と `FSM.scala:579-623` (名前付き timer) の対応表
  - **公開 API 変更 (追加のみ、BREAKING なし)**:
    - `FsmTransition::for_max(timeout: Option<Duration>) -> Self` (Duration::ZERO は cancel に正規化)
    - `FsmTransition::replying(reply: AnyMessage) -> Self`
    - `Fsm::start_single_timer` / `start_timer_at_fixed_rate` / `start_timer_with_fixed_delay` / `cancel_timer` / `is_timer_active`
    - `FsmTimerFired` 型 (`fsm::FsmTimerFired` として型だけ pub export、`new` / accessor は `pub(crate)` で内部利用専用、state handler には unwrap 済 payload が渡るためユーザコードが直接触ることはない)
  - **テスト**: `for_max` 5 件 + `replying` 3 件 + 名前付き timer 6 件 + stop-cleanup 1 件 + `FsmTimerFired` 3 件 = 新規 18 件
  - gap-analysis FS-M1 + FS-M2 done 化、新版 (Phase 1.12 で確定した `N+1`) で medium 減
- [ ] 10.3 レビュー対応: CodeRabbit / Cursor Bugbot 指摘は Pekko 互換を崩さない範囲で対応、却下する場合は理由を reply してから resolve
- [ ] 10.4 マージ後、別 PR で change をアーカイブ + main spec を `openspec/specs/pekko-fsm-transition-extensions/spec.md` に sync
