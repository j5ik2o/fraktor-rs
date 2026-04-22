## 1. 現状把握と事前調査

- [x] 1.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs:256,294` の
  `_throughput_deadline` 引数と "follow-up change" コメントを確認 (本 change で削除)
- [x] 1.2 `Mailbox::new(policy)` の呼び出し箇所を grep で全列挙
  (`modules/actor-core/` + `modules/actor-adaptor-std/` + tests + showcases)
- [x] 1.3 Pekko `Mailbox.scala:261-278` を開き、`processMailbox` / `shouldProcessMessage` /
  `isThroughputDeadlineTimeDefined` / `throughputDeadlineTime` の契約を行単位で再確認

## 2. `MailboxClock` 型 alias と clock 配送経路の整備 (kernel 層)

- [x] 2.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_clock.rs` を新設
  - `pub type MailboxClock = alloc::sync::Arc<dyn Fn() -> core::time::Duration + Send + Sync>;`
  - rustdoc で Pekko `System.nanoTime()` 互換 + monotonic 必須の契約を明記
  - no_std (`alloc`) のみ依存、`std::time::Instant` 等は adaptor 側に残す
- [x] 2.2 `MailboxSharedSet` (kernel `modules/actor-core/src/core/kernel/system/shared_factory/mailbox_shared_set.rs`)
  に `clock: Option<MailboxClock>` field を追加 (design Decision 1 の factory 方針)
  - **None sentinel = deadline enforcement 無効 (throughput-only fallback)**。no_std core で
    `builtin()` が呼ばれても panic なし
  - **破壊的変更: `MailboxSharedSet::new` の `const fn` 修飾子を削除** (`Arc<dyn Fn()>` 含む
    `Option<MailboxClock>` を初期化する const-context コンストラクタは Rust stable で
    構築不可能なため、通常の `pub(crate) fn new(...)` に降格。`put_lock` 単独時の const fn
    利点はなくなるが、既存の `const fn` 呼び出し箇所 (grep 検証必須) は本 change で破壊的に修正)
  - `MailboxSharedSet::builtin()` は `clock = None` で構築 (従来挙動と完全一致)
  - `MailboxSharedSet::with_clock(self, clock: MailboxClock) -> Self` の builder を追加
    (`#[must_use]`、std adaptor の `ActorSystem` 初期化経路で注入)
    - **重複呼び出し契約**: 既に `clock = Some(_)` の bundle に再度 `.with_clock(another)` を
      呼んだ場合は **新しい clock で上書き** (panic しない、builder pattern の自然な挙動)。
      production 経路では `ActorSystem` 初期化で一度のみ呼ぶが、テスト / embedded adaptor で
      差し替える可能性があるため rustdoc に明記
  - `MailboxSharedSet::clock(&self) -> Option<&MailboxClock>` の getter を追加
- [x] 2.3 `Mailbox` 構造体に `clock: Option<MailboxClock>` field を追加
  (`Option<Arc<dyn Fn()>>` は Clone 可能。既存 `Mailbox` は Clone 派生なし / `SyncOnce` 制約あり)
  - 既存 `Mailbox` (`base.rs:34-60`) は `#[derive(Debug)]` を持たないため追加作業不要
  - **将来 `Debug` を追加する際の罠防止**: `Option<Arc<dyn Fn() -> Duration + Send + Sync>>` は
    `Debug` auto-derive **不可** (`dyn Fn()` trait object は `Debug` 未実装) のため、
    将来 `#[derive(Debug)]` を付けるとコンパイルエラーになる。本 field のドキュメンテーション
    コメント (英語 rustdoc) に `// NOTE: does not implement Debug; manual impl required if
    derive(Debug) is ever added` と明記
  - 既存の `unsafe impl Send for Mailbox {}` `unsafe impl Sync for Mailbox {}` (`base.rs:62-63`)
    は `Option<Arc<dyn Fn() + Send + Sync>>` で引き続き成立
- [x] 2.4 **既存 8 本の factory コンストラクタは signature 維持** (`base.rs:68-187`):
  - `Mailbox::new(policy)` / `new_with_shared_set` / `new_from_config(_with_shared_set)`
  - `Mailbox::new_sharing(policy, queue)` / `new_sharing_with_shared_set`
  - `Mailbox::with_actor(actor, policy, queue)` / `with_actor_and_shared_set`
  - 最終的な集約関数 `new_with_queue_and_shared_set(policy, queue, shared_set)` で
    `mailbox.clock = shared_set.clock().cloned();` を埋める (clock=None ならそのまま None)
- [x] 2.5 clock 差し替え API を追加:
  - `pub(crate) fn set_clock(&mut self, clock: Option<MailboxClock>)` (CQS 原則 Command、`&mut self + ()`)
  - 可視性: **`pub(crate)`** (kernel 内部 + テスト経由のみ)。`actor-adaptor-std` は
    `MailboxSharedSet::with_clock()` + factory 経由で clock を注入するため、`set_clock` の
    外部公開は不要
  - immutability-policy に則り builder `self -> Self` は **不採用** (`SyncOnce` が Clone 不可のため
    field 再構築コスト回避)。rustdoc で CQS exception 扱いでないことを明記

## 3. deadline enforcement の実装 (kernel 層 `process_mailbox`)

- [x] 3.1 `Mailbox::run()` 内で `_throughput_deadline` の `_` を削除、実名 `throughput_deadline` へ
- [x] 3.2 `run()` 先頭で一度だけ
  `let deadline_at: Option<Duration> = self.clock.as_ref().zip(throughput_deadline).map(|(c, d)| c() + d);`
  を評価 (design Decision 2)。`self.clock = None` または `throughput_deadline = None` のいずれかで
  `deadline_at = None` となり deadline 判定がスキップされる
- [x] 3.3 `process_mailbox(invoker, throughput)` のシグネチャを
  `process_mailbox(invoker, throughput, deadline_at: Option<Duration>)` に変更
- [x] 3.4 ループ条件に deadline 判定を追加。**Pekko `Mailbox.scala:271-276` の順序を厳守**:
  ```rust
  while left > 0 && self.should_process_message() {
    let Some(envelope) = self.dequeue() else { break; };
    // Pekko L271: actor.invoke(next)
    invoker.with_write(|i| i.invoke(envelope.into_payload()));
    // Pekko L274: processAllSystemMessages()
    self.process_all_system_messages(invoker);
    // fraktor-rs は post-decrement の while で left > 0 を評価 (= Pekko の left > 1 で再帰と等価)
    left -= 1;
    // Pekko L275: (left > 1) && (!deadlineDefined || (nanoTime - deadlineNs) < 0)
    // deadline break は process_all_system_messages の **後** に置く (Pekko 順序準拠)
    if let Some(da) = deadline_at
        && self.clock.as_ref().is_some_and(|c| c() >= da)
    {
      break;
    }
  }
  ```
  - **deadline break で `process_all_system_messages` がスキップされてはならない**
    (Pekko L274 は L275 の `if` より先に評価される)
  - 各行に Pekko `Mailbox.scala:<L番号>` 参照を rustdoc で付与
- [x] 3.5 `// Deadline support is added in a follow-up change (MB-M1, Phase A3)` コメントを
  Pekko 行単位対応 rustdoc に差し替え

## 4. 呼び出し経路の整備 (factory signature は維持)

**実装順序:** 4.1 → **4.2 (monotonic closure helper 定義)** → **4.3 (std adaptor で clock 注入)** → 4.4
(Phase 2.2 の `with_clock` 完了が前提、4.2 で定義した helper を 4.3 が呼ぶ構造)

- [x] 4.1 `message_dispatcher_shared.rs:305` の `mbox_clone.run(throughput, throughput_deadline)`
  呼び出しをそのまま維持 (`run()` signature 不変)。**完了条件**:
  - `mbox_clone` が保持する `Mailbox.clock` 値が `MailboxSharedSet::with_clock()` 経由で
    注入された clock と一致することを `grep` で `clock` 配送経路を追跡 (`mailbox_shared_set.rs` →
    `Mailbox::new_with_queue_and_shared_set` → `mbox_clone`)
  - Phase 5.2 / 5.10 の実行時テストで deadline enforcement が `throughput_deadline = Some(_)`
    時に実効的に動いていることが確認できる (clock が正しく届いていなければテスト失敗)
  - `run()` signature 変更が本 change 完了時に **発生していない** ことを grep gate 7.1-7.6 で確認
- [x] 4.2 **Instant::now() ベースの新規 closure helper** を `modules/actor-adaptor-std/` に
  定義 (**既存 `SystemState::monotonic_now` は AtomicU64 カウンタで実時間非対応のため流用不可**):
  - 既存 `pub fn monotonic_now(&self) -> Duration` (system_state.rs:863) は `fetch_add(1)` を
    毎回呼び `Duration::from_millis(ticks)` を返す instrumentation 用カウンタであり、
    wall-clock 経過時間を返さない。これは throughput deadline の経過時間判定には機能しない
    (throughput=100, deadline=10ms でカウンタが 10 以下なら永遠に deadline 未達)
  - **新規 helper**: `std_monotonic_mailbox_clock() -> MailboxClock`
    - 起動時の `Instant` を capture (`let start = Instant::now();`)
    - closure 内で `start.elapsed()` を返す (`Arc::new(move || start.elapsed())`)
    - `ActorSystem` への強参照/Weak 参照を closure に含めず、純粋な `Instant` capture のみで
      cycle を回避 (Weak 参照方式は design Decision 1 で却下済み、本 task では採用しない)
  - core 側は `MailboxClock` 型 alias のみ、`Instant` 依存は std adaptor に閉じる
    (`cfg-std-forbid` dylint が core での `std::time` 参照を検出)
- [x] 4.3 `MailboxSharedSet::with_clock()` 経由で std adaptor から clock を注入する factory:
  - `SystemState` / `SystemStateShared` に `mailbox_shared_set: MailboxSharedSet` field を追加
    (`new()` / builder で初期化、default は clock=None)
  - std adaptor (`modules/actor-adaptor-std/`) の `ActorSystem` 初期化時に 4.2 の
    `std_monotonic_mailbox_clock()` を呼び、`MailboxSharedSet::builtin().with_clock(clock)`
    で clock 付き bundle を構築して system に install
  - `actor_cell.rs:156` の `Mailbox::new_from_config(&mailbox_config)` を
    `Mailbox::new_from_config_with_shared_set(&cfg, &system.mailbox_shared_set())` に置き換え
  - `balancing_dispatcher.rs:109` 等の kernel 内 builtin() 直接呼び出しも system 経由に統一
  - no_std env (core 単体ビルド) では system にデフォルトで clock=None、embedded adaptor が
    独自 clock を install
- [x] 4.4 `Mailbox::new(policy)` / `new_with_shared_set` / `new_from_config(_with_shared_set)` /
  `new_sharing(_with_shared_set)` / `with_actor(_and_shared_set)` の既存呼び出し箇所は
  **signature 維持のため書き換え不要** (破壊的書き換えなし):
  - kernel 内部の factory (`MessageDispatcherShared` 等) は `MailboxSharedSet` 経由で clock 受信
  - テストコードで mock clock を使う場合は `mailbox.set_clock(Some(mock_clock))` 呼び出しを追加
    (Phase 2.5 で定義した `Mailbox::set_clock(&mut self, Option<MailboxClock>)`)
  - showcases / integration tests は `MailboxSharedSet::builtin()` の default clock (None) で十分
    (deadline enforcement は follow-up change で有効化)

## 5. 契約 pinned テストの追加 (kernel mailbox `tests.rs`)

- [x] 5.1 mock clock utility を追加: `SpinSyncMutex<Duration>` ラッパー + `Arc<dyn Fn() -> Duration + Send + Sync>`
  にキャストする helper (`MockClock::new(start)` + `MockClock::advance(delta)` + `MockClock::as_mailbox_clock()`)
- [x] 5.2 テスト `throughput_deadline_expired_yields_before_exhausting_throughput`:
  - GIVEN throughput=100, deadline=10ms, 各 invoke 中に clock を 5ms 進める
  - 100 通積む → `run()` を呼ぶ → yield 時点で **処理数 < 100** を検証
  - reschedule 要求 (`run()` 戻り値) が `true`
- [x] 5.3 テスト `throughput_deadline_none_processes_all_throughput`:
  - GIVEN throughput=100, deadline=None, 各 invoke 中に clock を 5ms 進める
  - 100 通積む → `run()` を呼ぶ → **100 通すべて処理** を検証 (throughput-only 挙動)
- [x] 5.4 テスト `throughput_limit_takes_precedence_when_deadline_far_in_future`:
  - GIVEN throughput=10, deadline=60s, 軽量 invoke (clock 進まず)
  - 20 通積む → `run()` → **10 通で yield** (throughput 上限で抜ける)
- [x] 5.5 テスト `deadline_computed_once_per_run`:
  - 各 iteration で clock を進めつつ、deadline 判定が「run 先頭 + deadline」を使い続けることを検証
  - ループ途中で clock が deadline を超えた瞬間に yield される (再計算されない)
- [x] 5.6 テスト `monotonic_clock_resilience_to_wallclock_rewind` (doc scenario 相当):
  - mock clock を前進 → 巻き戻し → 前進と動かす (wallclock simulation)
  - 内部 `deadline_at` は monotonic `Duration` として保持されるため、巻き戻しに影響されないことを確認
- [x] 5.7 テスト `throughput_1_with_deadline_zero_yields_after_one_message` (Pekko `left > 1` 境界):
  - GIVEN throughput=1, deadline=Some(Duration::ZERO), mock clock 任意 (進行あり/なし両方)
  - 10 通積む → `run()` → 1 通処理で yield (throughput=1 消化、deadline break 経路には到達しない)
- [x] 5.8 テスト `throughput_2_with_deadline_zero_and_fixed_clock` (deadline 境界動作):
  - GIVEN throughput=2, deadline=Some(Duration::ZERO), mock clock 固定 (advance 呼ばない)
  - 5 通積む → `run()` → 1 通処理後 `clock_now >= deadline_at` で break
  - 合計処理数 = 1 (throughput=2 の 2 通目に到達しない = deadline 判定が効いている証拠)
- [x] 5.9 テスト `clock_none_falls_back_to_throughput_only`:
  - GIVEN `MailboxSharedSet::builtin()` の clock=None、throughput=10, deadline=Some(10ms)
  - 各 invoke で手動 advance 1sec (通常なら deadline 超過) しても 10 通すべて処理される
  - clock=None の fallback が正しく効いていることを確認
- [x] 5.10 テスト `deadline_zero_with_clock_progress_breaks_after_one_message` (spec R1 Scenario
  "deadline = Some(Duration::ZERO) は 1 件処理後に break する (clock 進行あり)" 対応):
  - GIVEN throughput=10, deadline=Some(Duration::ZERO), mailbox に 10 通積む
  - 各 invoke で `mock.advance(Duration::from_micros(1))` を呼ぶ
  - WHEN `run()` を呼ぶ
  - THEN 1 通処理後に `clock_now >= deadline_at` (= `elapsed_run_start + 0`) が成立して break
  - AND 合計処理数 = 1、reschedule 要求 `true`
  - **throughput ≥ 2 + clock 進行あり + deadline=ZERO** の組み合わせで deadline break 経路が
    実効的に動くことを pin する (5.7 は throughput=1 境界、5.8 は clock 固定、5.10 は clock 進行あり)

## 6. Pekko 行単位 rustdoc の追加

- [x] 6.1 `process_mailbox` の新ループ条件に Pekko `Mailbox.scala:275` への参照をコメント付与
- [x] 6.2 `run()` 先頭の deadline 計算に Pekko `Mailbox.scala:263-266` への参照を付与
- [x] 6.3 `MailboxClock` 定義の rustdoc に Pekko `System.nanoTime()` 互換 + monotonic 必須
  (wall-clock 禁止) を明記
- [x] 6.4 `// Deadline support is added in a follow-up change (MB-M1, Phase A3)` の古いコメントを
  削除し、Pekko 行単位対応 rustdoc と置換

## 7. 機械的検証 (grep gate)

- [x] 7.1 `grep -rn "_throughput_deadline: Option" modules/actor-core/src/` が **0 件**
  (未使用引数プレフィックス削除済。`with_throughput_deadline(...)` method 名の一致は対象外)
- [x] 7.2 `grep -rn "Deadline support is added in a follow-up change" modules/` が **0 件**
- [x] 7.3 `grep -rn "MailboxClock" modules/` で **21 件** (定義 1 + 使用多数 ≥ 4)
- [x] 7.4 `grep -rn "Option<MailboxClock>" modules/actor-core/src/` で **4 箇所**
  (`MailboxSharedSet` field + `Mailbox` field + `set_clock` / `install_mailbox_clock` 引数)
- [x] 7.5 `grep -n "fn set_clock\b\|fn with_clock\b\|fn clock\b" modules/actor-core/src/core/kernel/`
  で `Mailbox::set_clock`, `MailboxSharedSet::with_clock`, `MailboxSharedSet::clock` がすべて存在
- [x] 7.6 `grep -n "const fn new" modules/actor-core/src/core/kernel/system/shared_factory/mailbox_shared_set.rs`
  が **0 件** (Phase 2.2 で `const fn` を通常 fn に降格済を確認)

## 8. gap-analysis 更新

- [x] 8.1 `docs/gap-analysis/actor-gap-analysis.md` の MB-M1 行を
  `~~medium~~ done (change `pekko-mailbox-throughput-deadline`)` に書き換え
- [x] 8.2 第 12 版を追加、medium カウントを 12 → 11 に更新、change log に 2026-04-22 エントリ追加

## 9. Pekko 参照検証 (実装完了時の行単位突合)

- [x] 9.1 `references/pekko/.../Mailbox.scala:261-278` の `processMailbox` と本実装の
  `process_mailbox` を行単位突合、rustdoc `// Pekko ...` コメントで対応を明示
- [x] 9.2 Pekko `System.nanoTime + deadlineNs` の加算が
  `self.clock.as_ref().zip(throughput_deadline).map(|(c, d)| c() + d)` と
  セマンティクス的に一致することを design.md / rustdoc 双方で確認
- [x] 9.3 本 change で Pekko 非互換を新たに作っていないことを検証:
  - 既存テスト全 passing (workspace)
  - 本 change で `#[ignore]` 新規付与なし

## 10. CI / lint の final ゲート

- [x] 10.1 `openspec validate pekko-mailbox-throughput-deadline --strict` が valid を返す
- [x] 10.2 `./scripts/ci-check.sh ai all` が exit 0
  - dylint 8 lint 全 pass (特に `cfg-std-forbid`: core 側で `std::time` を使っていないこと)
  - cargo test / clippy / fmt が全て pass

## 11. PR 作成 / マージ / アーカイブ

- [x] 11.1 `feat(actor-core): enforce mailbox throughput deadline (MB-M1)` という題で PR を作成、
  本 change の change name をリンク
- [x] 11.2 PR 本文に以下を含める:
  - Pekko `Mailbox.scala:261-278` との行単位対応表
  - 公開 API 変更: `MailboxSharedSet` への `clock` field 追加 (既存 `Mailbox::new*` signature は維持)
  - `MailboxClock` 導入による clock 注入設計の根拠 (design.md Decision 1 の要約 +
    factory 経路集約方針)
  - gap-analysis MB-M1 done 化の反映
- [x] 11.3 レビュー対応: CodeRabbit / Cursor Bugbot の指摘が来た場合は Pekko 互換を崩さない
  範囲で対応、却下する場合は理由を reply してから resolve
  (CodeRabbit 7 件 + Cursor Bugbot 3 件すべて対応・resolve 済み)
- [x] 11.4 マージ後、別 PR で change をアーカイブ (PR #1631 squash merge 2026-04-22)
