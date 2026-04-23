## Why

Pekko `Mailbox.scala:844,931` には `BoundedDequeBasedMailbox` と `BoundedControlAwareMailbox` の bounded variant が存在し、以下の 2 ユースケースを解放する:

1. **Stash + Bounded**: deque-capable mailbox (`needs_deque()`) を要求する actor (stash / `unstash_first` 等を使うもの) を、容量制約付きで運用したい場合。Pekko の `BoundedDequeBasedMailbox` が該当。
2. **ControlAware + Bounded**: 制御メッセージ優先 (`needs_control_aware()`) の actor を容量制約付きで運用したい場合。Pekko の `BoundedControlAwareMailbox` が該当。

fraktor-rs 現状 (`modules/actor-core/src/core/kernel/dispatch/mailbox/`):
- Unbounded 側は 5 variant (plain / deque / control_aware / priority / stable_priority) 揃っている
- Bounded 側は 3 variant (plain / priority / stable_priority) のみ
- **deque + bounded**: `mailboxes.rs:89` の `deque_mailbox_type_from_policy` が `MailboxConfigError::BoundedWithDeque` で fail-fast し、組合せ自体を禁止
- **control_aware + bounded**: `MailboxConfig::validate()` (`mailbox_config.rs:137-141`) が `MailboxConfigError::ControlAwareRequiresUnboundedPolicy` を返して fail-fast で拒否しており、組合せ自体が unvalid。さらに `mailboxes.rs:54-57` の `create_message_queue_from_config` は `needs_control_aware()` を検出しても **無条件に Unbounded 版**を生成 (capacity 分岐なし) のため、validate を迂回する経路があっても実際の capacity 制約は効かない

本 change で 2 variant を新設し、mailboxes.rs の分岐を bounded 対応に拡張する。gap-analysis MB-M2 は第16版で残存 medium 5 件の 1 つとして特定されており、既存 pattern の複製で確実に閉塞可能と評価されている。

## What Changes

- **新規型**: `BoundedDequeMailboxType` + `BoundedDequeMessageQueue` を追加。既存 `UnboundedDequeMessageQueue` (`VecDeque` + `DequeMessageQueue` trait) と `BoundedMessageQueue` (capacity + `MailboxOverflowStrategy`) の意味論を合成する。
- **新規型**: `BoundedControlAwareMailboxType` + `BoundedControlAwareMessageQueue` を追加。既存 `UnboundedControlAwareMessageQueue` の dual-queue (control / normal 2 本の VecDeque) に capacity + overflow strategy を被せる。
- **分岐拡張** (`mailboxes.rs`):
  - `deque_mailbox_type_from_policy`: `Bounded { capacity }` 枝を `BoundedDequeMailboxType::new(capacity, overflow)` に差し替え、`Err(BoundedWithDeque)` の fail-fast を廃止
  - `create_message_queue_from_config` の control-aware 分岐: 現状の無条件 `UnboundedControlAwareMailboxType` を `policy.capacity()` で `Bounded` / `Unbounded` 振分け
- **BREAKING**: `MailboxConfigError::BoundedWithDeque` variant を削除 (新 BoundedDeque 型で組合せが valid になるため error 自体が無意味)。caller (`validate` / tests) も追随更新する。
- **BREAKING**: `MailboxConfigError::ControlAwareRequiresUnboundedPolicy` variant を削除 (新 BoundedControlAware 型で組合せが valid になるため)。`mailbox_config.rs:137-141` の validate 分岐と rustdoc (L125-126) も撤去。caller (`mailbox_config/tests.rs:56` など) も追随更新する。
- **テスト**: 新 mailbox type 2 件ごとに既存 `bounded_message_queue/tests.rs` / `unbounded_deque_message_queue/tests.rs` / `unbounded_control_aware_message_queue/tests.rs` と同パターンの unit tests を追加。overflow strategy 3 種 (`Grow` / `DropNewest` / `DropOldest`) × 新 variant 2 種で `BoundedMessageQueue` と等価な挙動を検証。
- **gap-analysis 更新**: 第17版として MB-M2 を done 化、残存 medium を 4 件 (AC-M2, AC-M4b [deferred], FS-M1, FS-M2) に更新。

## Capabilities

### New Capabilities
- `pekko-bounded-deque-control-aware-mailbox`: Pekko `BoundedDequeBasedMailbox` / `BoundedControlAwareMailbox` と等価な bounded mailbox variant を提供する契約。Stash / ControlAware を bounded 下で運用できることを保証する。

### Modified Capabilities
<!-- 該当なし: 既存 mailbox capability は variant 列挙を spec 化していない (内部実装詳細のため)。本 change で新規 capability として Bounded 組合せの契約を確立する。 -->

## Impact

**影響を受けるコード**:
- `modules/actor-core/src/core/kernel/dispatch/mailbox/` 直下に 4 新規ファイル + 2 tests モジュール:
  - `bounded_deque_mailbox_type.rs` (+ `bounded_deque_mailbox_type/tests.rs`)
  - `bounded_deque_message_queue.rs` (+ `bounded_deque_message_queue/tests.rs`)
  - `bounded_control_aware_mailbox_type.rs` (+ `bounded_control_aware_mailbox_type/tests.rs`)
  - `bounded_control_aware_message_queue.rs` (+ `bounded_control_aware_message_queue/tests.rs`)
- `modules/actor-core/src/core/kernel/dispatch/mailbox.rs` に新 mod 登録 4 件
- `modules/actor-core/src/core/kernel/dispatch/mailbox/mailboxes.rs`: `deque_mailbox_type_from_policy` / control-aware 分岐の書換え
- `modules/actor-core/src/core/kernel/actor/props/mailbox_config.rs` / `mailbox_config_error.rs`: `BoundedWithDeque` + `ControlAwareRequiresUnboundedPolicy` validation の撤去
- `modules/actor-core/src/core/kernel/actor/props/mailbox_config/tests.rs`: 既存 2 variant 期待テスト (`BoundedWithDeque` / `ControlAwareRequiresUnboundedPolicy`) の差替え
- 関連 `mailboxes/tests.rs` / `actor/props/base/tests.rs` / `typed/props/tests.rs`: 新 variant の組合せカバー追加と既存 assertion 反転

**影響を受ける API 契約**:
- **BREAKING**: `MailboxConfigError::BoundedWithDeque` variant 削除 (9 参照 / 6 ファイル)。public enum variant なので再 export している下流も影響。fraktor-rs 内部では `validate()` の戻り値型と数箇所のテストのみ影響を受ける。
- **BREAKING**: `MailboxConfigError::ControlAwareRequiresUnboundedPolicy` variant 削除 (5 参照 / 3 ファイル)。
- `MailboxConfig::validate()` の成功範囲拡張: 従来 `bounded + deque` / `bounded + control_aware` は `Err` を返していたが、本 change 以降は `Ok(())` を返し、`create_message_queue_from_config` が対応する新 Bounded 型を返す。
- `create_message_queue_from_config`: 従来 validate で fail-fast されていた control-aware + bounded 組合せが、本 change 以降は validate 成功 + `BoundedControlAwareMessageQueue` 生成として通る (behavior fix)。

**影響を受けないもの**:
- `MessageQueue` / `DequeMessageQueue` trait 定義
- `MailboxPolicy` / `MailboxCapacity` / `MailboxOverflowStrategy` / `MailboxRequirement` 定義
- 既存 Unbounded variant 全て / 既存 Bounded plain-priority / stable-priority variant
- `Envelope` / `QueueStateHandle` / dead-letter routing
- Pekko `Mailbox.scala:844,931` 以外の mailbox 関連 API (本 change は bounded variant 追加のみ)

**テスト** (spec Requirement 1/2/3 の Scenario と 1:1 対応):
- `BoundedDequeMessageQueue`: 6 件 (Grow 容量超過 / DropNewest 容量超過 / DropOldest 容量超過 / enqueue_first × DropNewest / enqueue_first × DropOldest (Decision 2-c Reject) / clean_up)
- `BoundedControlAwareMessageQueue`: 5 件 (control priority dequeue / DropOldest × normal front evict / DropOldest × normal 空時 Reject / DropNewest / Grow)
- `mailboxes.rs` dispatch 分岐の回帰テスト 2 件 (bounded + deque / bounded + control_aware)
- `mailbox_config/tests.rs` の `BoundedWithDeque` / `ControlAwareRequiresUnboundedPolicy` 期待テスト 2 件を `Ok(())` 期待に rename + 反転
- 既存全テストが pass することを regression 確認

**gap-analysis**:
- MB-M2 行を done 化 (第17版)
- 残存 medium 5 → 4 件 (AC-M2, AC-M4b [deferred], FS-M1, FS-M2)
