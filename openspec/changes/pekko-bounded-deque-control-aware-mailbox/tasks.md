## Phase 1: 準備と参照確認

- [ ] 1.1 既存 `BoundedMessageQueue` (`bounded_message_queue.rs`) の enqueue match 分岐 (Grow / DropNewest / DropOldest) と `offer` / `offer_if_room` / `offer_after_dropping_oldest` helper を `rtk read` で確認し、overflow handling パターンを特定
- [ ] 1.2 既存 `UnboundedDequeMessageQueue` の `DequeMessageQueue::enqueue_first` 実装を確認し、push_front に overflow strategy を適用する方針を再確認 (本 change 用に拡張)
- [ ] 1.3 既存 `UnboundedControlAwareMessageQueue` の dual-queue 構造と `is_control()` 判定経路を確認
- [ ] 1.4 `mailbox.rs` (mod エントリ) の既存 mod 宣言順を確認 (新 mod 4 件を alphabetical に挿入)
- [ ] 1.5 `mailboxes.rs` の `deque_mailbox_type_from_policy` と `create_message_queue_from_config` の control-aware 分岐の現行コードを確認
- [ ] 1.6 `mailbox_config.rs::validate` (L137-148) で両拒否分岐 (`BoundedWithDeque` + `ControlAwareRequiresUnboundedPolicy`) を確認し、Phase 5A/5B の削除範囲を確定
- [ ] 1.7 `rtk grep "BoundedWithDeque|ControlAwareRequiresUnboundedPolicy" --glob "*.rs"` で全参照を列挙 (想定 14 参照 / 7 ファイル前後)

## Phase 2: BoundedDeque variant の追加

- [ ] 2.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_deque_message_queue.rs` を新規作成:
  - `pub struct BoundedDequeMessageQueue { inner: SharedLock<VecDeque<Envelope>>, capacity: usize, overflow: MailboxOverflowStrategy }`
  - `new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy) -> Self`
  - `impl MessageQueue`: `enqueue` / `dequeue` / `number_of_messages` / `clean_up` / `as_deque`
  - `impl DequeMessageQueue`: `enqueue_first`
  - enqueue は overflow 分岐 (Grow = push_back, DropNewest = len check + Rejected, DropOldest = pop_front evict + push_back + Evicted)
  - **enqueue_first (Decision 2-c)**: Grow = push_front + Ok, DropNewest = len check + Err(SendError::Full), **DropOldest = len check + Err(SendError::Full)** (evict しない Reject 方式)
- [ ] 2.2 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_deque_message_queue/tests.rs` を新規作成: spec の 6 シナリオに対応するテスト 6 件以上
  - Grow で 3 件 enqueue 成功
  - DropNewest の Rejected
  - DropOldest の Evicted (front evict)
  - enqueue_first の DropNewest で Err(SendError::Full)
  - **enqueue_first の DropOldest も Err(SendError::Full) (既存 entry を evict しない)**
  - clean_up で全 clear
- [ ] 2.3 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_deque_mailbox_type.rs` を新規作成:
  - `pub struct BoundedDequeMailboxType { capacity: NonZeroUsize, overflow: MailboxOverflowStrategy }`
  - `new(capacity, overflow) -> Self`
  - `impl MailboxType`: `create(&self) -> Box<dyn MessageQueue>`
- [ ] 2.4 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_deque_mailbox_type/tests.rs` を新規作成: factory が BoundedDequeMessageQueue を生成することの最低限の検証 (既存 `bounded_mailbox_type/tests.rs` パターン)

## Phase 3: BoundedControlAware variant の追加

- [ ] 3.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_control_aware_message_queue.rs` を新規作成:
  - `pub struct BoundedControlAwareMessageQueue { inner: SharedLock<Inner>, capacity: usize, overflow: MailboxOverflowStrategy }` + `struct Inner { control_queue: VecDeque<Envelope>, normal_queue: VecDeque<Envelope> }`
  - `new(capacity, overflow) -> Self`
  - `impl MessageQueue`: `enqueue` / `dequeue` / `number_of_messages` / `clean_up`
  - enqueue 内の容量判定: `control_queue.len() + normal_queue.len() >= capacity`
  - overflow 分岐:
    - Grow: 対応 queue に push_back、Accepted
    - DropNewest: capacity 超過なら Rejected(envelope)
    - DropOldest: normal_queue が空でないなら front を evict して対応 queue に push_back、Evicted(evicted); normal_queue が空なら control drop を避けて Rejected (Decision 3)
  - dequeue: `control_queue.pop_front().or_else(|| normal_queue.pop_front())`
- [ ] 3.2 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_control_aware_message_queue/tests.rs` を新規作成: spec の 5 シナリオに対応するテスト 5 件以上
  - control 優先 dequeue
  - DropOldest の normal 優先 evict
  - DropOldest + normal 空 → control Reject
  - DropNewest の Rejected
  - Grow で capacity 超過受理
- [ ] 3.3 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_control_aware_mailbox_type.rs` を新規作成:
  - `pub struct BoundedControlAwareMailboxType { capacity, overflow }`
  - `new(capacity, overflow) -> Self`
  - `impl MailboxType`: `create`
- [ ] 3.4 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_control_aware_mailbox_type/tests.rs` を新規作成

## Phase 4: mod 宣言と dispatch 分岐の更新

- [ ] 4.1 `modules/actor-core/src/core/kernel/dispatch/mailbox.rs` に以下を追加 (alphabetical な位置に):
  - `pub(crate) mod bounded_control_aware_mailbox_type;`
  - `pub(crate) mod bounded_control_aware_message_queue;`
  - `pub(crate) mod bounded_deque_mailbox_type;`
  - `pub(crate) mod bounded_deque_message_queue;`
- [ ] 4.2 `mailboxes.rs` の imports に `BoundedDequeMailboxType`, `BoundedControlAwareMailboxType` を追加
- [ ] 4.3 `mailboxes.rs::deque_mailbox_type_from_policy` を書換:
  ```rust
  match policy.capacity() {
    MailboxCapacity::Bounded { capacity } => Box::new(BoundedDequeMailboxType::new(capacity, policy.overflow())),
    MailboxCapacity::Unbounded => Box::new(UnboundedDequeMailboxType::new()),
  }
  ```
  戻り値を `Box<dyn MailboxType>` に変更し、`Result` / `MailboxConfigError` を返さなくする
- [ ] 4.4 `mailboxes.rs::create_message_queue_from_config` の control-aware 分岐を書換:
  ```rust
  if config.requirement().needs_control_aware() {
    let mailbox_type: Box<dyn MailboxType> = match config.policy().capacity() {
      MailboxCapacity::Bounded { capacity } => {
        Box::new(BoundedControlAwareMailboxType::new(capacity, config.policy().overflow()))
      }
      MailboxCapacity::Unbounded => Box::new(UnboundedControlAwareMailboxType::new()),
    };
    return Ok(mailbox_type.create());
  }
  ```
- [ ] 4.5 `mailboxes.rs::create_message_queue_from_config` の deque 分岐 (`config.requirement().needs_deque()` 部分) で、`deque_mailbox_type_from_policy` が Result を返さなくなるのに追随して `?` を削除
- [ ] 4.6 `mailboxes/tests.rs` を調べて、`Err(BoundedWithDeque)` を期待していた箇所があれば `Ok(BoundedDequeMessageQueue 生成)` 期待に差替え。新 variant の dispatch 回帰テスト 2 件を追加 (bounded + deque / bounded + control_aware)

## Phase 5: `MailboxConfigError::BoundedWithDeque` + `ControlAwareRequiresUnboundedPolicy` の削除

**背景** (ultrareview merged_bug_001 で判明): `MailboxConfig::validate()` には 2 つの関連拒否分岐があり、`BoundedWithDeque` だけでなく `ControlAwareRequiresUnboundedPolicy` も削除する必要がある。後者を残すと新 Bounded+ControlAware 分岐が unreachable dead code になる。

### 5A: `BoundedWithDeque` variant の削除 (9 参照 / 6 ファイル)

- [ ] 5.1 `modules/actor-core/src/core/kernel/actor/props/mailbox_config_error.rs` から `BoundedWithDeque` variant を削除 (L14) + `Display` impl の対応 arm 削除 (L33)
- [ ] 5.2 `modules/actor-core/src/core/kernel/actor/props/mailbox_config.rs::validate` L148 付近の `return Err(MailboxConfigError::BoundedWithDeque);` 枝を削除。関連 rustdoc (L131) も更新
- [ ] 5.3 `modules/actor-core/src/core/kernel/actor/props/mailbox_config/tests.rs` L93 の `BoundedWithDeque` 期待を `Ok(())` 期待に差替え。テスト名も `rejects` → `accepts` に rename
- [ ] 5.4 **修正**: `modules/actor-core/src/core/kernel/actor/props/base/tests.rs` L76 の `BoundedWithDeque` 期待を `Ok(())` 期待に差替え (※旧 tasks は `dispatch/mailbox/base/tests.rs` と誤記、実際は `actor/props/base/tests.rs`)
- [ ] 5.5 `modules/actor-core/src/core/typed/props/tests.rs:46` の `with_stash_mailbox_rejects_bounded_mailbox_config` を `with_stash_mailbox_accepts_bounded_mailbox_config` に rename、assertion を `Err(BoundedWithDeque)` → `Ok(())` に反転 (stash + bounded は本 change で valid 組合せに)

### 5B: `ControlAwareRequiresUnboundedPolicy` variant の削除 (5 参照 / 3 ファイル)

- [ ] 5.6 `modules/actor-core/src/core/kernel/actor/props/mailbox_config_error.rs` から `ControlAwareRequiresUnboundedPolicy` variant を削除 (L10) + `Display` impl の対応 arm 削除 (L27)
- [ ] 5.7 `modules/actor-core/src/core/kernel/actor/props/mailbox_config.rs::validate` L137-141 の `needs_control_aware() && Bounded` 拒否分岐を削除。関連 rustdoc (L125-126) も更新
- [ ] 5.8 `modules/actor-core/src/core/kernel/actor/props/mailbox_config/tests.rs::validate_rejects_control_aware_with_bounded_policy` (L56 付近) を `validate_accepts_control_aware_with_bounded_policy` に rename、assertion を `Err(ControlAwareRequiresUnboundedPolicy)` → `Ok(())` に反転

### 5C: 残参照ゼロ検証

- [ ] 5.9 `rtk grep "BoundedWithDeque"` で残参照ゼロを確認
- [ ] 5.10 `rtk grep "ControlAwareRequiresUnboundedPolicy"` で残参照ゼロを確認

## Phase 6: テストと CI 検証

- [ ] 6.1 `rtk cargo test -p fraktor-actor-core-rs --lib` で全テスト pass 確認。新 variant のテスト 10+ 件 + 既存 regression がすべて通ること
- [ ] 6.2 `rtk cargo test -p fraktor-actor-core-rs --tests` でインテグレーションテスト pass 確認
- [ ] 6.3 `./scripts/ci-check.sh ai all` を実行し exit 0 を確認
- [ ] 6.4 clippy / rustdoc / type-per-file lint で新規警告ゼロを確認

## Phase 7: gap-analysis 更新

- [ ] 7.1 `docs/gap-analysis/actor-gap-analysis.md` のサマリーテーブルに第17版 entry を追加:
  - `内部セマンティクスギャップ数 (第17版、MB-M2 完了反映後)` — `4+（high 0 / medium 4 / low 約 11）` + 残存 list
- [ ] 7.2 MB-M2 行 (`| MB-M2 | BoundedDequeBasedMailbox / BoundedControlAwareMailbox | ...`) を done 化:
  - `✅ **完了 (change `pekko-bounded-deque-control-aware-mailbox`)** —` プレフィックス
  - 実装参照を `bounded_deque_mailbox_type.rs` / `bounded_control_aware_mailbox_type.rs` に書換え
  - 最終列を `~~medium~~ done` に
- [ ] 7.3 Phase A3 セクションの「完了済み」リストに MB-M2 を追加
- [ ] 7.4 Phase A3 セクションの「残存 medium 5 件」を「残存 medium 4 件: AC-M2, AC-M4b (deferred), FS-M1, FS-M2」に更新
- [ ] 7.5 第10版時点の履歴記述末尾に第17版の追記を追加

## Phase 8: PR 発行とレビュー対応

- [ ] 8.1 branch `impl/pekko-bounded-deque-control-aware-mailbox` を切って PR 発行、base は main
- [ ] 8.2 PR 本文に以下を含める:
  - Pekko `Mailbox.scala:844,931` との対応表
  - **公開 API 変更**: `MailboxConfigError` から 2 variant 削除 (BREAKING):
    - `BoundedWithDeque` (bounded + deque が valid に)
    - `ControlAwareRequiresUnboundedPolicy` (bounded + control_aware が valid に)
  - **挙動変更**: control_aware + bounded は従来 `validate()` で fail-fast 拒否されていたが、新 variant により validate 成功 + BoundedControlAware 生成の整合パスに統一 (behavior fix)
  - **テスト**: 新 variant 10+ 件 + dispatch 回帰 2 件 + validate 差替え (2 テスト rename + assertion 反転)
  - gap-analysis MB-M2 done 化、第17版 medium 5 → 4
- [ ] 8.3 レビュー対応: CodeRabbit / Cursor Bugbot の指摘が来た場合は Pekko 互換を崩さない範囲で対応、却下する場合は理由を reply してから resolve
- [ ] 8.4 マージ後、別 PR で change をアーカイブ + main spec を `openspec/specs/pekko-bounded-deque-control-aware-mailbox/spec.md` に sync
