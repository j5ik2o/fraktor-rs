## Context

### 既存 mailbox 構造 (コード調査結果)

- `dispatch/mailbox/` 直下に mailbox variant が展開されており、各 variant は `*_mailbox_type.rs` (factory) + `*_message_queue.rs` (実装) の 2 ファイル構成
- **Unbounded 5 variant**: plain / deque / control_aware / priority / stable_priority
- **Bounded 3 variant**: plain / priority / stable_priority
- 欠落 2 variant: **BoundedDeque** / **BoundedControlAware**

### 2 系統の実装スタイル

`BoundedMessageQueue` (`bounded_message_queue.rs`):
- `QueueStateHandle<Envelope>` (高水準 queue abstraction) を使用
- `MailboxPolicy::bounded(capacity, overflow, None)` で policy 構築 → `QueueStateHandle::new_user(&policy)`
- enqueue は `overflow` に応じて `offer` / `offer_if_room` / `drop_oldest_and_offer` を分岐

`UnboundedDequeMessageQueue` / `UnboundedControlAwareMessageQueue`:
- `SharedLock<VecDeque<Envelope>>` を直接使用 (QueueStateHandle を経由しない)
- シンプルな `push_back` / `pop_front` のみ
- `UnboundedDeque` は `DequeMessageQueue` trait を impl (`enqueue_first` = push_front 提供)
- `UnboundedControlAware` は dual-queue (`control_queue` / `normal_queue`) で control 優先 dequeue

### 意味論の合成ポイント

BoundedDeque / BoundedControlAware で既存 pattern を再利用するには:

- **BoundedDeque** = Unbounded の `VecDeque` + `DequeMessageQueue` trait + **容量チェック + overflow strategy**
- **BoundedControlAware** = Unbounded の dual-queue + **合計サイズでの容量チェック + overflow strategy**

`QueueStateHandle` + bounded policy を使う手もあるが、`DequeMessageQueue::enqueue_first` (push_front) は QueueStateHandle の低水準 API に映らないため、本 change では直接 `VecDeque` + `SharedLock` で実装する。`BoundedMessageQueue` と実装スタイルは分かれるが、mailbox trait 契約と overflow strategy は同等。

### mailboxes.rs の dispatch 現状

```rust
fn deque_mailbox_type_from_policy(policy: MailboxPolicy) -> Result<Box<dyn MailboxType>, MailboxConfigError> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { .. } => Err(MailboxConfigError::BoundedWithDeque),  // ← MB-M2 の fail-fast
    | MailboxCapacity::Unbounded => Ok(Box::new(UnboundedDequeMailboxType::new())),
  }
}

// create_message_queue_from_config:
if config.requirement().needs_control_aware() {
  let mailbox_type: Box<dyn MailboxType> = Box::new(UnboundedControlAwareMailboxType::new());  // ← bounded 無視
  return Ok(mailbox_type.create());
}
```

本 change で上記 2 箇所を bounded 対応に拡張する。

## Goals / Non-Goals

**Goals:**
- Pekko `Mailbox.scala:844` `BoundedDequeBasedMailbox` / `Mailbox.scala:931` `BoundedControlAwareMailbox` と意味論的に等価な bounded mailbox variant を提供する
- Stash + bounded / ControlAware + bounded の組合せが `MailboxConfig::validate()` で pass し、`create_message_queue_from_config` が適切な queue type を返す
- `MailboxOverflowStrategy::Grow` / `DropNewest` / `DropOldest` 3 戦略すべてを新 variant でサポートする
- 既存 Unbounded / Bounded variant の挙動は破壊しない

**Non-Goals:**
- `priority + deque` や `priority + control_aware` のようなさらなる組合せは対象外 (Pekko にも無い)
- `BoundedControlAwareDequeMailbox` 相当 (3 軸の組合せ) も対象外 (Pekko にも無く YAGNI)
- `QueueStateHandle` 基盤への統一リファクタ — 既存 Unbounded 側も直接 VecDeque 方式なので本 change は踏襲する。統一は別 change で検討
- `MailboxOverflowStrategy::Fail` のような新戦略追加 — 既存 3 種のみサポート

## Decisions

### Decision 1: 実装基盤は `SharedLock<VecDeque>` 方式を踏襲する (QueueStateHandle は使わない)

- **選択**: 既存 `UnboundedDequeMessageQueue` / `UnboundedControlAwareMessageQueue` と同じ `SharedLock<VecDeque<Envelope>>` を使う。capacity 判定と overflow handling は自前で実装する。
- **Rationale**:
  - `DequeMessageQueue::enqueue_first` は `push_front` を要求するが、`QueueStateHandle` の低水準 API に push_front 経路が存在しない (offer / drop_oldest_and_offer のみ)
  - Control-aware 側は dual-queue 構造で QueueStateHandle 1 個では表現できない (2 本の queue が必要)
  - 既存 Unbounded 版と同系統の書き方に揃えることで読みやすさを維持
- **代替**: 3 系統 (QueueStateHandle ベース / 統一リファクタ / 本選択) を検討したが、既存 Unbounded 版踏襲が最小変更で確実。統一は将来 refactor で扱う

### Decision 2: overflow strategy の実装は `BoundedMessageQueue` のロジックを引き継ぐ

- **選択**: `BoundedMessageQueue` の `enqueue` match arm (DropNewest / DropOldest / Grow) と等価な分岐を新 variant に書き起こす。Shared helper 関数は抽出せず、variant ごとに直接書く (ファイル間依存の最小化)。
- **Rationale**:
  - DropOldest は `oldest` をどこから evict するかが variant 固有 (deque なら front、control-aware なら normal-queue 側 front を優先)。shared helper 化しても分岐が増え可読性低下
  - variant 数は 2 に限定されるので重複コードは許容範囲
  - `EnqueueOutcome` / `EnqueueError` / `DropOldestOutcome` 等の型は既存 API を使用
- **代替**: overflow handling を trait / macro 化して共通化する案は却下 (Decision 1 で論じた通り push_front / dual-queue の variant 固有差分を隠せない)

### Decision 3: BoundedControlAware の `DropOldest` は normal queue を優先 evict する

- **選択**: 容量超過時の DropOldest は **control_queue ではなく normal_queue の front** を evict する。両方空で capacity オーバーする状況は overflow check の性質上発生しない (`total_len < capacity` 前提)。
- **Rationale**:
  - Pekko の `BoundedControlAwareMailbox` も control 優先、normal drop の契約 (制御メッセージが drop されると supervision / PoisonPill が失われ、Actor 停止不能になる)
  - Control が優先される意味論上、容量確保は常に normal 側から取る方が期待どおり
- **エッジケース**: 到着 envelope が control で normal_queue が空の場合、control_queue 末尾から evict する frontier が必要。Pekko `BoundedControlAwareMailbox` はこの場合 control を drop せず fail する (=`EnqueueOutcome::Rejected`) — fraktor-rs も同仕様に倣い、normal が空のとき制御 enqueue は capacity 判定で Reject する (実装を設計 spec に記載)

### Decision 4: `MailboxConfigError::BoundedWithDeque` variant を削除する

- **選択**: `bounded + deque` が valid な組合せになるため、該当 variant と `validate()` 内の拒否ロジックをまとめて撤去する。related tests (`mailbox_config/tests.rs`) も `Ok(())` 期待に差替え。
- **Rationale**:
  - `BoundedWithDeque` の意味は「組合せを許容していない」ため残すと誤情報化する
  - CLAUDE.md 「後方互換は不要 / 破壊的変更を恐れずに最適な設計を追求」方針で variant 削除を許容
- **代替**: `BoundedWithDeque` を `#[deprecated]` で残す案は却下 (pre-release なので意味がない)

### Decision 5: `create_message_queue_from_config` の control-aware 分岐に capacity 判定を追加する

- **選択**: `needs_control_aware()` 枝の中で `policy.capacity()` を参照し、`Bounded { capacity }` なら `BoundedControlAwareMailboxType::new(capacity, overflow)`, `Unbounded` なら既存の `UnboundedControlAwareMailboxType::new()` を返す。
- **Rationale**:
  - 既存 priority 分岐 (`priority_mailbox_type_from_config`) と同じパターン
  - bounded 指定を silently unbounded fallback するのは gap (MB-M2 の根本原因の 1 つ) なので明示分岐で修正
- **代替**: ヘルパー関数 `control_aware_mailbox_type_from_policy` を切り出す案もあるが、既存 `deque_mailbox_type_from_policy` とパターン統一するためこちらで採用する

## Risks / Trade-offs

### Risk 1: BREAKING: `MailboxConfigError::BoundedWithDeque` variant 削除

- **影響**: public enum variant が削除される。下流 (workspace 内) で `match` していた箇所はコンパイルエラーになる。
- **範囲**: `rtk grep "BoundedWithDeque"` では fraktor-rs 内 7 箇所で参照されており、全て本 change で追随修正する対象。外部クレートへの波及はなし (pre-release)。
- **緩和**: CLAUDE.md 方針に従い破壊的変更を許容。tasks の caller 追随と CI 全通しで担保

### Risk 2: control-aware + bounded の silent unbounded fallback 修正による挙動変化

- **影響**: 従来 bounded を指定しても unbounded にフォールバックして受理していた config が、本 change 以降は実際に bounded として動作する。capacity 到達時に DropOldest / DropNewest 等の overflow 挙動が発生する。
- **範囲**: control-aware + bounded を指定する config が production で存在するかは不明。もし存在すれば実質的に silently buggy だった挙動が修正される。
- **緩和**: change log に BREAKING fix として明記。gap-analysis で MB-M2 の既知 issue として記録済

### Risk 3: DropOldest eviction 対象 (control vs normal) の判断ミス

- **影響**: Pekko と異なる eviction 戦略を採ると、supervision / PoisonPill の silent drop を招く。
- **緩和**: Decision 3 で normal 優先 evict を明示し、spec Scenario で Pekko 同等挙動を契約化。テストで control enqueue 時に normal evict されることを検証

### Risk 4: `QueueStateHandle` を使わないことによる metrics / instrumentation の欠落

- **影響**: `BoundedMessageQueue` は QueueStateHandle 経由で mailbox metrics (e.g. pressure event) を発火しうる。新 Bounded variant は直接 VecDeque なので同等の metrics 経路を持たない。
- **緩和**: 既存 `UnboundedDeque` / `UnboundedControlAware` も QueueStateHandle を使わない = metrics 経路は未配線。本 change は scope 外 (将来 `MailboxInstrumentation` の整備時に横断的に対応)。gap-analysis / tasks に記録

### Risk 5: 新 variant のテスト漏れによる overflow strategy 不整合

- **影響**: 3 strategy × 2 variant = 6 組合せのうちどれかで想定外挙動。例: BoundedControlAware の DropNewest が control を drop すると Pekko 互換を崩す。
- **緩和**: spec に各 strategy の期待 scenario を明示し、tests で全 6 組合せをカバー。特に control 優先の eviction 順序を明示検証

### Risk 6: 新 Bounded variant が deque trait の `enqueue_first` を `SendError::Full` に適切にマッピングしない

- **影響**: push_front が容量超過した際の挙動が DequeMessageQueue::enqueue_first の契約と整合しない可能性。
- **緩和**: push_front も capacity チェックを行い、容量超過時は overflow strategy に従う (Grow / DropNewest / DropOldest 同様)。`enqueue_first` の戻り値型は `Result<(), SendError>` なので DropOldest の evicted 情報は捨てる (trait 契約制約、上位は push_back 経由で enqueue するときのみ evicted を受け取る)
