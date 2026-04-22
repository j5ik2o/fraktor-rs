## 1. 現状把握 / 事前 grep gate

- [x] 1.1 `modules/actor-core/src/core/kernel/actor/actor_cell.rs:1527` の `ctx.reschedule_receive_timeout()` 呼び出し箇所と、周辺の invoke_user 成功ブランチを再確認する
- [x] 1.2 `modules/actor-core/src/core/kernel/actor/messaging/any_message.rs` の `is_control` field / `Clone` / `Debug` / `from_parts` / `into_parts` / `from_erased` の構造を確認し、`not_influence_receive_timeout: bool` を並列 field として追加した場合の touch 範囲を把握する (Phase 3.1-3.7 の先行確認)
- [x] 1.3 `AnyMessage::from_parts` / `AnyMessage::into_parts` / `AnyMessage::from_erased` の全 callers を grep で列挙。型名 prefix 付きで検索して他型の同名 method を除外 (`grep -rn "AnyMessage::from_parts\|AnyMessage::into_parts\|AnyMessage::from_erased\|\.from_parts(\|\.into_parts(\|\.from_erased(" modules/` で AnyMessage caller に絞る)。破壊的 signature 変更で影響する箇所を把握
- [x] 1.4 `AnyMessage::new(Identify::new(` / `AnyMessage::new(Identify{` 系の internal caller を grep で列挙 (`grep -rn "AnyMessage::new(Identify" modules/`)
- [x] 1.5 Pekko `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:165` (`NotInfluenceReceiveTimeout` trait) と `dungeon/ReceiveTimeout.scala:40-42,71-76` を開き、契約を行単位で再確認

## 2. `NotInfluenceReceiveTimeout` marker trait の新設

- [x] 2.1 `modules/actor-core/src/core/kernel/actor/messaging/not_influence_receive_timeout.rs` を新設
  - `pub trait NotInfluenceReceiveTimeout: Any + Send + Sync {}` を定義
  - rustdoc に Pekko `Actor.scala:165` 対応と「`AnyMessage::not_influence` 経由で送ることで flag を立てる」設計意図を記述
- [x] 2.2 `modules/actor-core/src/core/kernel/actor/messaging.rs` に `mod not_influence_receive_timeout;` を追加し、既存の re-export パターン (例: `pub use identify::Identify;` / `pub use any_message::AnyMessage;`) と整合させて `pub use not_influence_receive_timeout::NotInfluenceReceiveTimeout;` を記述 (dylint `mod-file` で `mod.rs` は禁止、`messaging.rs` ファイル方式で宣言)
- [x] 2.3 `modules/actor-core/src/core/kernel/actor/messaging/identify.rs` に以下を追加 (orphan rule により `Identify` 定義ファイル側に impl を置く):
  - 先頭に `use super::NotInfluenceReceiveTimeout;` (既存 `use` 群の位置に合わせる)
  - 末尾付近に `impl NotInfluenceReceiveTimeout for Identify {}` (empty impl、trait object 化不要)

## 3. `AnyMessage` への `not_influence_receive_timeout` flag 追加

> **Note**: Phase 3 と Phase 4 は同一コミット / 同一 PR 内で完了する必要がある。
> `AnyMessage::as_view` (Phase 4.6) が `AnyMessageView::with_flags` (Phase 4.4) に依存し、
> `AnyMessage` 側の field 拡張だけ先行すると build が壊れる。Phase 3 → Phase 4 の順で
> 実装しつつ、コミット粒度では両方完了まで commit しないこと。

- [x] 3.1 `AnyMessage` struct に `not_influence_receive_timeout: bool` field を追加 (`is_control` の直後に並べる)
- [x] 3.2 既存 `AnyMessage::new` / `::control` は `not_influence_receive_timeout: false` で構築するよう修正 (挙動不変)
- [x] 3.3 `AnyMessage::not_influence::<T: NotInfluenceReceiveTimeout + Any + Send + Sync + 'static>(payload) -> Self` コンストラクタを新設
  - trait bound で marker trait の実装を強制
  - rustdoc で以下を明記:
    - Pekko `isInstanceOf[NotInfluenceReceiveTimeout]` チェックの Rust 置換であること
    - **警告**: `impl NotInfluenceReceiveTimeout for T` を付けたとしても、`AnyMessage::new(value)` で封筒化した場合は flag が立たず timeout が reset される。marker を効かせるには **必ず** `AnyMessage::not_influence(value)` を経由すること
    - `AnyMessage::new` / `AnyMessage::control` 側の rustdoc にも「marker 効果を得るには `not_influence` を使う」旨の cross-reference を追加する
- [x] 3.4 `AnyMessage::is_not_influence_receive_timeout(&self) -> bool` 公開 getter を追加
- [x] 3.5 `Clone` 実装で `not_influence_receive_timeout` を伝播させる
- [x] 3.6 `Debug` 実装で `not_influence_receive_timeout` field を表示
- [x] 3.7 `AnyMessage::from_parts` / `AnyMessage::into_parts` / `AnyMessage::from_erased` の signature を拡張
  - 3-tuple → 4-tuple (`(payload, sender, is_control, not_influence_receive_timeout)`) へ
  - 破壊的変更: callers を Phase 1.3 の grep 結果から全て修正

## 4. `AnyMessageView` への同期

- [x] 4.1 `AnyMessageView` (`messaging/any_message_view.rs`) に `not_influence_receive_timeout: bool` field を追加
- [x] 4.2 `AnyMessageView::new` は `not_influence_receive_timeout: false` で構築 (既存挙動維持)
- [x] 4.3 `AnyMessageView::with_control(payload, sender, is_control)` は 3 引数で維持し、内部的に `not_influence_receive_timeout: false` を設定する (既存挙動完全維持、非破壊)
- [x] 4.4 `AnyMessageView::with_flags(payload, sender, is_control, not_influence_receive_timeout)` を新規追加 (4 引数のコンストラクタ)
- [x] 4.5 `AnyMessageView::not_influence_receive_timeout(&self) -> bool` 公開 getter を追加
- [x] 4.6 `AnyMessage::as_view` の実装を、4.4 で追加した `AnyMessageView::with_flags` を呼ぶ形に書き換え、`not_influence_receive_timeout` flag を view に渡す

## 5. `ActorCellInvoker::invoke` の reschedule ガード

- [x] 5.1 `actor_cell.rs:1527` 付近の `Ok(()) => ctx.reschedule_receive_timeout()` をガード付きに置換する。Phase 5.2 で選んだ A/B のいずれかに従って flag を参照する:
  ```rust
  // Phase 5.2 A 案 (既存 failure_candidate を再利用)
  | Ok(()) => {
    if !failure_candidate.is_not_influence_receive_timeout() {
      ctx.reschedule_receive_timeout();
    }
  },
  // Phase 5.2 B 案 (invoke 先頭で local 変数)
  | Ok(()) => {
    if !not_influence {
      ctx.reschedule_receive_timeout();
    }
  },
  ```
- [x] 5.2 `message` が `invoke_user` に move された後の参照問題を回避する。以下のいずれか:
  - **A (推奨)**: 既存 `let failure_candidate = message.clone();` (actor_cell.rs:1524) 経由で `failure_candidate.is_not_influence_receive_timeout()` を `Ok(())` ブランチで呼ぶ (追加 clone 無し、既存構造との整合性が高い)
  - **B**: invoke の先頭 (1523 手前) で `let not_influence = message.is_not_influence_receive_timeout();` を取り、`Ok(())` ブランチで local 変数を使う (既存 `failure_candidate` を触らない)
- [x] 5.3 Pekko `dungeon/ReceiveTimeout.scala:40-42` 準拠を rustdoc コメントで明示 (「`not_influence == true` のときは reschedule しない、Pekko の `!message.isInstanceOf[NotInfluenceReceiveTimeout]` 判定に相当」)
- [x] 5.4 失敗時 `Err(error)` ブランチは touch しない (既存挙動維持、`user_message_failure_does_not_reschedule_receive_timeout` テストが引き続き pass)

## 6. 内部 `Identify` 封筒化経路の修正

- [x] 6.1 `modules/actor-core/src/core/kernel/actor/actor_selection/selection.rs:77` の
  `AnyMessage::new(Identify::new(...))` を `AnyMessage::not_influence(Identify::new(...))` に変更
- [x] 6.2 Phase 1.4 の grep 結果を確認 (調査時点では selection.rs:77 の 1 件のみ)。万が一他経路が発見された場合は同様に書き換える。発見ゼロなら本 task はそのまま完了扱い
- [x] 6.3 `Identify` 応答の `ActorIdentity` 封筒化 (`actor_cell.rs:1517`) は `AnyMessage::new(identity)` のまま維持 (Pekko でも ActorIdentity は non-marker)

## 7. 契約 pinned テスト追加 (`actor_cell/tests.rs` + `any_message/tests.rs`)

> **配置方針**: 7.2/7.3/7.4/7.7 は actor_cell 依存なので `actor_cell/tests.rs`、7.5/7.6 は
> `AnyMessage` 単体の挙動なので `any_message/tests.rs` に分ける。

- [x] 7.1 テスト helper: ユーザー定義型 `struct NonInfluencingTick;` + `impl NotInfluenceReceiveTimeout for NonInfluencingTick {}` を test module に用意
- [x] 7.2 テスト `not_influence_message_skips_reschedule`:
  - GIVEN `ActorCell::create(...)` で actor を起動し、`pre_start` で `ctx.set_receive_timeout(Duration::from_millis(20), _)` を呼ぶ `ReceiveTimeoutNoopActor` を配置
  - AND 初期 schedule 完了後の `gen_before = cell.receive_timeout.as_shared_lock().with_lock(|state| state.as_ref().map(ReceiveTimeoutState::schedule_generation))` を読み取る
  - AND `AnyMessage::not_influence(NonInfluencingTick)` を `ActorCellInvoker::invoke` に渡す
  - WHEN `invoke` が `Ok(())` で完了
  - THEN invoke 後に再取得した generation が `gen_before` と等しい (差 0 = reschedule スキップされた)
- [x] 7.3 テスト `regular_message_reschedules_receive_timeout`:
  - GIVEN 7.2 と同じ手順で actor / `ctx` / timeout を用意
  - AND 初期 schedule 完了後の `gen_before` を読み取る
  - AND `AnyMessage::new(NonInfluencingTick)` (not_influence 無し) を `ActorCellInvoker::invoke` に渡す
  - WHEN `invoke` が `Ok(())` で完了
  - THEN invoke 後の generation が `gen_before + 1` (cancel + schedule 1 回分が走った)
- [x] 7.4 テスト `identify_message_is_not_influence_by_internal_path`:
  - 実装戦略: `actor_selection/selection.rs:77` の Identify 封筒化を `pub(crate)` の **test-visible helper** に切り出す。既存 `selection.rs` の style に応じて以下のいずれかを選択する:
    - (a) `impl ActorSelection` ブロック内の associated method: `pub(crate) fn to_identify_envelope(&self) -> AnyMessage`
    - (b) module-level free function: `pub(crate) fn build_identify_envelope(selection: &ActorSelection) -> AnyMessage`
  - GIVEN helper を呼び出して Identify を封筒化する
  - WHEN 返された `AnyMessage` を観察
  - THEN `is_not_influence_receive_timeout() == true`
- [x] 7.5 テスト `not_influence_flag_is_preserved_on_clone`:
  - GIVEN `let msg = AnyMessage::not_influence(NonInfluencingTick);`
  - WHEN `msg.clone()`
  - THEN clone 後も `is_not_influence_receive_timeout() == true`
- [x] 7.6 テスト `view_exposes_not_influence_flag`:
  - GIVEN `AnyMessage::not_influence(NonInfluencingTick).as_view()`
  - THEN `view.not_influence_receive_timeout() == true`
- [x] 7.7 既存テスト `user_message_failure_does_not_reschedule_receive_timeout` (`actor_cell/tests.rs:565` 付近) が依然 pass することを確認 (regression guard)
- [x] 7.8 `AnyMessage::not_influence` の rustdoc に **`compile_fail` doctest** を 1 件追加し、spec Requirement 1 Scenario 3 「marker 未実装型は封筒化できない = コンパイルエラー」契約を pin する:
  ```rust
  /// ```compile_fail,E0277
  /// use fraktor_actor_core_rs::core::kernel::actor::messaging::AnyMessage;
  /// struct RegularMsg;
  /// // RegularMsg は NotInfluenceReceiveTimeout を実装していないため trait bound で reject される
  /// let _ = AnyMessage::not_influence(RegularMsg);
  /// ```
  ```
  - `cargo test --doc` で rustc の型検査が働き、封筒化拒否が静的契約として pin される
  - **注意**: `use` path は実装時に `modules/actor-core/src/lib.rs` の re-export を確認し、`AnyMessage` の実際の public path に合わせる (prelude 経由 or 完全修飾 path のどちらか。crate 内 `pub use` 経路次第)

## 8. `receive_timeout_state` の generation counter 追加 (テスト + 診断用途)

- [x] 8.1 `receive_timeout_state.rs` に `schedule_generation: u64` field を追加 (初期値 0)
  - 既存 `ReceiveTimeoutState` の field 拡張 (破壊的 private 変更で OK、public API は不変)
- [x] 8.2 `ActorContext::schedule_receive_timeout` (private helper、`actor_context.rs:537` 付近) **内部の末尾で** `state.schedule_generation = state.schedule_generation.saturating_add(1);` を実行
  - `set_receive_timeout` (初期) / `reschedule_receive_timeout` (cancel + schedule) の両経路が共通して通る schedule helper で +1 する設計になっているため、generation は「schedule 呼び出し回数」と一致する
  - テストは「invoke 前の generation」と「invoke 後の generation」を比較して差分を検証する (差 0 = skip、差 1 = reschedule 発生)
  - 初期 `set_receive_timeout` で generation は 1 になる点は、テスト側で invoke 前の baseline として読み取るため問題なし
- [x] 8.3 `pub(crate) const fn schedule_generation(&self) -> u64` を `ReceiveTimeoutState` に追加 (kernel テスト読み取りおよび将来の production diagnostics 読み取りの両用、`#[cfg(test)]` ゲートなし)
- [x] 8.4 `pub fn receive_timeout_schedule_generation(&self) -> Option<u64>` を `ActorContext` に追加し、内部 `with_receive_timeout_slot_ref` 経由で state の generation を覗けるようにする (`None` は未設定時)。**public 可視性にすることで lib crate における reachability が確保され、dead_code 警告を発生させずに「production diagnostics hook」の設計意図を成立させる**

## 9. 機械的検証 (grep gate + CI)

- [x] 9.1 `grep -rn "AnyMessage::new(Identify" modules/` が **0 件** (全 not_influence 経路に移行済)
- [x] 9.2 `grep -rn "NotInfluenceReceiveTimeout" modules/` で **4 件以上** を確認:
  1. `messaging/not_influence_receive_timeout.rs` の `pub trait NotInfluenceReceiveTimeout` 定義
  2. `messaging/identify.rs` の `impl NotInfluenceReceiveTimeout for Identify {}`
  3. `messaging/any_message.rs` の `pub fn not_influence<T: NotInfluenceReceiveTimeout ...>(payload) -> Self` (trait bound 参照)
  4. テストファイル内の `impl NotInfluenceReceiveTimeout for NonInfluencingTick {}`
- [x] 9.3 `grep -rn "not_influence_receive_timeout" modules/` で次がすべて 1 件以上存在:
  - `any_message.rs` の field / `is_not_influence_receive_timeout` getter
  - `any_message_view.rs` の field / `not_influence_receive_timeout` getter / `with_flags` 引数
  - `actor_cell.rs` の invoke ガード
  - tests (`actor_cell/tests.rs` + `any_message/tests.rs`)
- [x] 9.4 `grep -rn "reschedule_receive_timeout\b" modules/actor-core/src/core/kernel/actor/actor_cell.rs` が invoke ガード付き 1 箇所 のみ (`actor_context.rs` 内の既存 `pub(crate) fn reschedule_receive_timeout` 定義は path 範囲外のため対象外)
- [x] 9.5 `openspec validate pekko-receive-timeout-not-influence --strict` が valid

## 10. gap-analysis 更新

- [x] 10.1 `docs/gap-analysis/actor-gap-analysis.md` の AC-M5 行を `~~medium~~ done (change pekko-receive-timeout-not-influence)` に書き換え
- [x] 10.2 第 13 版を追加、medium カウントを 11 → 10 に更新、change log に本 change のマージ日エントリを追加 (実装時の当日日付 `YYYY-MM-DD` を使用)

## 11. Pekko 参照検証

- [x] 11.1 `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:81,165` (L81: `Identify` の `NotInfluenceReceiveTimeout` mix-in、L165: trait 定義) を rustdoc から参照
- [x] 11.2 `references/pekko/actor/src/main/scala/org/apache/pekko/actor/dungeon/ReceiveTimeout.scala:40-42` (出口側 `checkReceiveTimeoutIfNeeded` 判定) を `actor_cell.rs:1527` 付近の rustdoc から参照。入口側 `:71-76` (`cancelReceiveTimeoutIfNeeded`) は本 change で実装しない (design Decision 3) ため、本 change の rustdoc 参照からは除外する (将来入口 cancel 導入時に追加)
- [x] 11.3 本 change で Pekko 非互換を新たに作っていないことを確認:
  - 既存 `actor_context/tests.rs:811,847` の `set_receive_timeout(Duration::from_millis(20), AnyMessage::new(ReceiveTimeoutTick))` テストが引き続き pass (`ReceiveTimeoutTick` は marker 無しのためフラグ false、従来通り reschedule される)
  - 既存テスト全 passing (workspace) — 1806 lib tests + 7 doctests 全 pass
  - 本 change で `#[ignore]` 新規付与なし

## 12. CI / lint の final ゲート

- [x] 12.1 `./scripts/ci-check.sh ai all` が exit 0
  - dylint 8 lint 全 pass (特に `cfg-std-forbid`: core 側で `std::time` 等を使っていないこと)
  - cargo test / clippy / fmt が全 pass

## 13. PR 作成 / マージ / アーカイブ

- [x] 13.1 `feat(actor-core): introduce NotInfluenceReceiveTimeout marker (AC-M5)` という題で PR を作成、本 change の change name をリンク → PR #1633
- [x] 13.2 PR 本文に以下を含める:
  - Pekko `Actor.scala:165` (trait) + `Actor.scala:81` (`Identify` mix-in) + `dungeon/ReceiveTimeout.scala:40-42` (出口側判定) との対応表。入口側 `:71-76` は本 change では実装しない旨を明記 (design Decision 3)
  - **公開 API 追加 (additive)**: `NotInfluenceReceiveTimeout` trait、`AnyMessage::not_influence`、`AnyMessage::is_not_influence_receive_timeout`、`AnyMessageView::not_influence_receive_timeout`、`AnyMessageView::with_flags`、`ActorContext::receive_timeout_schedule_generation`
  - **公開 API 破壊的変更 (BREAKING)**: `AnyMessage::from_parts` / `AnyMessage::into_parts` / `AnyMessage::from_erased` の tuple 要素数が 3 → 4 に拡張
  - **非破壊 (参考)**: `AnyMessage::new` / `AnyMessage::control` / `AnyMessageView::new` / `AnyMessageView::with_control` の signature は不変
  - gap-analysis AC-M5 done 化の反映
- [ ] 13.3 レビュー対応: CodeRabbit / Cursor Bugbot の指摘が来た場合は Pekko 互換を崩さない範囲で対応、却下する場合は理由を reply してから resolve
- [ ] 13.4 マージ後、別 PR で change をアーカイブ
