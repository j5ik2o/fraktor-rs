## Phase 1: 準備と参照確認

- [x] 1.1 既存 `BoundedMessageQueue` (`bounded_message_queue.rs`) の enqueue match 分岐 (Grow / DropNewest / DropOldest) と `offer` / `offer_if_room` / `offer_after_dropping_oldest` helper を `rtk read` で確認し、overflow handling パターンを特定
- [x] 1.2 既存 `UnboundedDequeMessageQueue` の `DequeMessageQueue::enqueue_first` 実装を確認し、push_front に overflow strategy を適用する方針を再確認 (本 change 用に拡張)
- [x] 1.3 既存 `UnboundedControlAwareMessageQueue` の dual-queue 構造と `is_control()` 判定経路を確認
- [x] 1.4 `mailbox.rs` (mod エントリ) の既存 mod 宣言順を確認 (新 mod 4 件を alphabetical に挿入)
- [x] 1.5 `mailboxes.rs` の `deque_mailbox_type_from_policy` と `create_message_queue_from_config` の control-aware 分岐の現行コードを確認
- [x] 1.6 `mailbox_config.rs::validate` (L137-148) で両拒否分岐 (`BoundedWithDeque` + `ControlAwareRequiresUnboundedPolicy`) を確認し、Phase 5A/5B の削除範囲を確定
- [x] 1.7 `rtk grep "BoundedWithDeque|ControlAwareRequiresUnboundedPolicy" --glob "*.rs"` で全参照を列挙 (想定 14 参照 / unique 6 ファイル、design Risk 1 参照)

## Phase 2: BoundedDeque variant の追加

- [x] 2.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_deque_message_queue.rs` を新規作成。実装は spec Requirement 1 (enqueue/dequeue/enqueue_first の overflow 契約) に従う。`UnboundedDequeMessageQueue` の内部構造を踏襲し、`BoundedMessageQueue` の overflow 分岐を混ぜる (design Decision 1, 2, 2-c)
- [x] 2.2 `bounded_deque_message_queue/tests.rs` を新規作成。spec Requirement 1 の 6 Scenario に 1:1 対応するテストを追加 (`UnboundedDequeMessageQueue::tests` と `BoundedMessageQueue::tests` のパターン混合)
- [x] 2.3 `bounded_deque_mailbox_type.rs` を新規作成 (既存 `BoundedMailboxType` と同パターン、MessageQueue 生成先だけ差し替え)
- [x] 2.4 `bounded_deque_mailbox_type/tests.rs` を新規作成 (既存 `bounded_mailbox_type/tests.rs` パターン: factory が正しい型を生成することの最低限検証)
- [x] 2.5 `./scripts/ci-check.sh ai dylint` を実行し、新規ファイルで dylint エラーゼロを確認 (特に type-per-file / mod-file / module-wiring / use-placement / rustdoc / cfg-std-forbid / ambiguous-suffix / tests-location) — **Phase 4.8 で統合実行**: 新規ファイルは mod 宣言 (Phase 4.1) 前は dylint 対象外となるため、Phase 4 の mod 登録後にまとめて走らせる

## Phase 3: BoundedControlAware variant の追加

- [x] 3.1 `bounded_control_aware_message_queue.rs` を新規作成。実装は spec Requirement 2 (control 優先 dequeue + 合計 capacity 強制 + normal 優先 evict) に従う。`UnboundedControlAwareMessageQueue` の dual-queue 構造を踏襲し、合計 length による overflow 判定と `DropOldest` 時の normal-優先 evict / normal 空時の Reject を追加 (design Decision 1, 3)
- [x] 3.2 `bounded_control_aware_message_queue/tests.rs` を新規作成。spec Requirement 2 の 5 Scenario に 1:1 対応するテストを追加
- [x] 3.3 `bounded_control_aware_mailbox_type.rs` を新規作成 (既存 `BoundedMailboxType` と同パターン、生成先 MessageQueue を `BoundedControlAwareMessageQueue` に差し替え)
- [x] 3.4 `bounded_control_aware_mailbox_type/tests.rs` を新規作成 (最低限の factory 検証)
- [x] 3.5 `./scripts/ci-check.sh ai dylint` を実行し、新規ファイルで dylint エラーゼロを確認 — **Phase 4.8 で統合実行**

## Phase 4: mod 宣言と dispatch 分岐の更新

- [x] 4.1 `mailbox.rs` に 4 新 mod (`bounded_deque_message_queue` / `bounded_deque_mailbox_type` / `bounded_control_aware_message_queue` / `bounded_control_aware_mailbox_type`) の宣言と `pub use` 再 export を追加。既存 `bounded_*` / `unbounded_*` mod と同じ 3 段パターン (`/// doc` + `mod ...;` + `pub use ...::Type;`) に揃える
- [x] 4.2 `mailboxes.rs` の imports に新 MailboxType 2 種を追加
- [x] 4.3 `mailboxes.rs::deque_mailbox_type_from_policy` を書換: 戻り値型を `Box<dyn MailboxType>` に変更 (Result を剥がす)、`Bounded` 分岐で `BoundedDequeMailboxType` を返す
- [x] 4.4 `mailboxes.rs` に helper `control_aware_mailbox_type_from_policy(policy) -> Box<dyn MailboxType>` を新設 (既存 `deque_mailbox_type_from_policy` / `priority_mailbox_type_from_config` と同形)。`create_message_queue_from_config` の control-aware 枝から helper を呼ぶよう書換 (design Decision 5)
- [x] 4.5 `create_message_queue_from_config` 内の `deque_mailbox_type_from_policy` 呼び出し箇所で、戻り値型変更に追随して `?` を除去
- [x] 4.6 `mailboxes/tests.rs` L78 の `create_message_queue_rejects_bounded_with_deque` を `create_message_queue_creates_bounded_deque_for_bounded_plus_deque` 等に rename し、assertion を `Ok(BoundedDequeMessageQueue)` 相当の検証 (例: `number_of_messages == 0` 直後の enqueue が DropNewest で期待通り挙動) に差替え
- [x] 4.7 新規 dispatch 回帰テストを `mailboxes/tests.rs` に追加: `create_message_queue_creates_bounded_control_aware_for_bounded_plus_control_aware` — bounded + control_aware config で `BoundedControlAwareMessageQueue` が生成され、capacity を超えた enqueue が Rejected/Evicted として挙動することを確認
- [x] 4.8 `./scripts/ci-check.sh ai dylint` を実行し、dispatch 書換え後の dylint エラーゼロを確認 (mod 宣言順 / use-placement / module-wiring の変化を検知) — Phase 6.3 の `ai all` 内で検証済

## Phase 5: `MailboxConfigError::BoundedWithDeque` + `ControlAwareRequiresUnboundedPolicy` の削除

**背景** (ultrareview merged_bug_001 で判明): `MailboxConfig::validate()` には 2 つの関連拒否分岐があり、`BoundedWithDeque` だけでなく `ControlAwareRequiresUnboundedPolicy` も削除する必要がある。後者を残すと新 Bounded+ControlAware 分岐が unreachable dead code になる。

**削除対象外の拒否分岐** (本 change で触らない):
- `MailboxConfigError::PriorityWithControlAware` (priority + control_aware 組合せ)
- `MailboxConfigError::PriorityWithDeque` (priority + deque 組合せ)
- `MailboxConfigError::DequeWithControlAware` (deque + control_aware 組合せ)
- `MailboxConfigError::StablePriorityWithoutGenerator`

これらは独立した組合せ制約であり、MB-M2 の scope (bounded + {deque, control_aware} 許容) と直交する。

### 5A: `BoundedWithDeque` variant の削除 (9 参照 / 6 ファイル)

- [x] 5.1 `modules/actor-core/src/core/kernel/actor/props/mailbox_config_error.rs` から `BoundedWithDeque` variant を削除 (L14) + `Display` impl の対応 arm 削除 (L33)
- [x] 5.2 `modules/actor-core/src/core/kernel/actor/props/mailbox_config.rs::validate` L145-149 の `needs_deque() && Bounded` 拒否分岐 (if ブロック全体) を削除。関連 rustdoc (L131-132) も更新
- [x] 5.3 `modules/actor-core/src/core/kernel/actor/props/mailbox_config/tests.rs` L93 の `BoundedWithDeque` 期待を `Ok(())` 期待に差替え。テスト名も `rejects` → `accepts` に rename
- [x] 5.4 **修正**: `modules/actor-core/src/core/kernel/actor/props/base/tests.rs` L67 の `with_stash_mailbox_rejects_bounded_mailbox_config` を `with_stash_mailbox_accepts_bounded_mailbox_config` に rename、L76 の assertion を `Err(BoundedWithDeque)` → `Ok(())` に反転 (※旧 tasks は `dispatch/mailbox/base/tests.rs` と誤記、実際は `actor/props/base/tests.rs`)
- [x] 5.5 `modules/actor-core/src/core/typed/props/tests.rs:46` の `with_stash_mailbox_rejects_bounded_mailbox_config` を `with_stash_mailbox_accepts_bounded_mailbox_config` に rename、assertion を `Err(BoundedWithDeque)` → `Ok(())` に反転 (stash + bounded は本 change で valid 組合せに)

### 5B: `ControlAwareRequiresUnboundedPolicy` variant の削除 (5 参照 / 3 ファイル)

- [x] 5.6 `modules/actor-core/src/core/kernel/actor/props/mailbox_config_error.rs` から `ControlAwareRequiresUnboundedPolicy` variant を削除 (L10) + `Display` impl の対応 arm 削除 (L27)
- [x] 5.7 `modules/actor-core/src/core/kernel/actor/props/mailbox_config.rs::validate` L137-141 の `needs_control_aware() && Bounded` 拒否分岐を削除。関連 rustdoc (L125-126) も更新
- [x] 5.8 `modules/actor-core/src/core/kernel/actor/props/mailbox_config/tests.rs::validate_rejects_control_aware_with_bounded_policy` (L56 付近) を `validate_accepts_control_aware_with_bounded_policy` に rename、assertion を `Err(ControlAwareRequiresUnboundedPolicy)` → `Ok(())` に反転

### 5C: 残参照ゼロ検証

- [x] 5.9 `rtk grep "BoundedWithDeque" modules/ --glob "*.rs"` で残参照ゼロを確認 (※ openspec/changes/pekko-bounded-deque-control-aware-mailbox/ 自身の言及と archive 配下は immutable history として対象外、modules/ 配下のみ検証)
- [x] 5.10 `rtk grep "ControlAwareRequiresUnboundedPolicy" modules/ --glob "*.rs"` で残参照ゼロを確認 (同上の範囲限定)
- [x] 5.11 `./scripts/ci-check.sh ai dylint` を実行し、variant 削除後の dylint エラーゼロを確認 (削除 variant の残参照を dylint 側からも検知) — Phase 6.3 の `ai all` 内で検証済

## Phase 6: テストと CI 検証

- [x] 6.1 `rtk cargo test -p fraktor-actor-core-rs --lib` で全テスト pass 確認。新 variant のテスト 10+ 件 + 既存 regression がすべて通ること — 1841 件 pass
- [x] 6.2 `rtk cargo test -p fraktor-actor-core-rs --tests` でインテグレーションテスト pass 確認 — 1915 件 pass
- [x] 6.3 `./scripts/ci-check.sh ai all` を実行し exit 0 を確認
- [x] 6.4 clippy / rustdoc / type-per-file lint で新規警告ゼロを確認 — `ai all` 内で dylint + clippy + doc がすべて pass

## Phase 7: gap-analysis 更新

- [x] 7.1 `docs/gap-analysis/actor-gap-analysis.md` のサマリーテーブルに第17版 entry を追加:
  - `内部セマンティクスギャップ数 (第17版、MB-M2 完了反映後)` — `4+（high 0 / medium 4 / low 約 11）` + 残存 list
- [x] 7.2 MB-M2 行 (`| MB-M2 | BoundedDequeBasedMailbox / BoundedControlAwareMailbox | ...`) を done 化:
  - `✅ **完了 (change `pekko-bounded-deque-control-aware-mailbox`)** —` プレフィックス
  - 実装参照を `bounded_deque_mailbox_type.rs` / `bounded_control_aware_mailbox_type.rs` に書換え
  - 最終列を `~~medium~~ done` に
- [x] 7.3 Phase A3 セクションの「完了済み」リストに MB-M2 を追加
- [x] 7.4 Phase A3 セクションの「残存 medium 5 件」を「残存 medium 4 件: AC-M2, AC-M4b (deferred), FS-M1, FS-M2」に更新
- [x] 7.5 第10版時点の履歴記述末尾に第17版の追記を追加

## Phase 8: PR 発行とレビュー対応

- [x] 8.1 branch `impl/pekko-bounded-deque-control-aware-mailbox` を切って PR 発行、base は main — PR #1642
- [x] 8.2 PR 本文に以下を含める:
  - Pekko `Mailbox.scala:844,931` との対応表
  - **公開 API 変更**: `MailboxConfigError` から 2 variant 削除 (BREAKING):
    - `BoundedWithDeque` (bounded + deque が valid に)
    - `ControlAwareRequiresUnboundedPolicy` (bounded + control_aware が valid に)
  - **挙動変更**: control_aware + bounded は従来 `validate()` で fail-fast 拒否されていたが、新 variant により validate 成功 + BoundedControlAware 生成の整合パスに統一 (behavior fix)
  - **テスト**: 新 variant 12 件 (BoundedDeque 7 + BoundedControlAware 5) + factory 系 4 件 + dispatch 回帰 2 件 (rename 1 + new 1) + 既存 validate test rename + assertion 反転 4 件 (Phase 5.3 / 5.4 / 5.5 / 5.8)
  - gap-analysis MB-M2 done 化、第17版 medium 5 → 4
- [ ] 8.3 レビュー対応: CodeRabbit / Cursor Bugbot の指摘が来た場合は Pekko 互換を崩さない範囲で対応、却下する場合は理由を reply してから resolve — レビューコメント着次第対応
- [ ] 8.4 マージ後、別 PR で change をアーカイブ + main spec を `openspec/specs/pekko-bounded-deque-control-aware-mailbox/spec.md` に sync — マージ後に実施
