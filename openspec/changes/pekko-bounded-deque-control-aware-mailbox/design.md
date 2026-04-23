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
  - `EnqueueOutcome` / `EnqueueError` / `SendError` 等の公開型は既存 API を使用 (内部 `QueueStateHandle` / `DropOldestOutcome` は Decision 1 により使わない)
- **代替**: overflow handling を trait / macro 化して共通化する案は却下 (Decision 1 で論じた通り push_front / dual-queue の variant 固有差分を隠せない)

### Decision 2-c: `BoundedDequeMessageQueue::enqueue_first` の DropOldest は **Reject** で扱う

- **問題**: `enqueue_first` (push_front) は stash rehydration 用途で、Pekko `BoundedDequeBasedMailbox` には明示規定が無い。DropOldest をどう適用するかは設計判断。
- **検討した選択肢**:
  - (a) **front evict**: `enqueue` 側と同じ front evict。しかし push_front で front に入れるため、自分が push した envelope を直後に evict する可能性が高く意味を成さない
  - (b) **back evict**: push_front に対し back を evict。"oldest" の語義 (長く保持している方 = 通常 front) と外れるため混乱を招く
  - (c) **Reject**: `Err(SendError::Full(envelope))` を返し、既存 entry もいずれも evict しない
- **選択**: (c) Reject 方式。
- **Rationale**:
  - DropNewest + enqueue_first と同じ挙動に揃うので一貫性が高い
  - stash rehydration 失敗を呼び出し側が `?` で catch できる
  - "oldest の evict 対象が push_front と push_back で逆転する" という spec-level の語義不整合を回避
- **代替**: (a), (b) は ultrareview 指摘 (bug_008) で語義ねじれが明示されたため却下
- **Pekko 参照**: 該当組合せの明示規定無し、fraktor-rs 独自 divergence として spec に明記

### Decision 3: BoundedControlAware の `DropOldest` は normal queue を優先 evict する

- **選択**: 容量超過時の DropOldest は **control_queue ではなく normal_queue の front** を evict する。両方空で capacity オーバーする状況は overflow check の性質上発生しない (`total_len < capacity` 前提)。
- **Rationale**:
  - Pekko の `BoundedControlAwareMailbox` も control 優先、normal drop の契約 (制御メッセージが drop されると supervision / PoisonPill が失われ、Actor 停止不能になる)
  - Control が優先される意味論上、容量確保は常に normal 側から取る方が期待どおり
- **エッジケース**: 到着 envelope が control で normal_queue が空の場合、control_queue 末尾から evict する frontier が必要。Pekko `BoundedControlAwareMailbox` はこの場合 control を drop せず fail する (=`EnqueueOutcome::Rejected`) — fraktor-rs も同仕様に倣い、normal が空のとき制御 enqueue は capacity 判定で Reject する (実装を設計 spec に記載)

### Decision 4: `MailboxConfigError::BoundedWithDeque` / `ControlAwareRequiresUnboundedPolicy` variant を削除する

- **背景調査** (ultrareview merged_bug_001 指摘で判明):
  - `MailboxConfig::validate()` には **2 つの関連拒否分岐**が存在する:
    - `mailbox_config.rs:145-148` (L148 で `BoundedWithDeque` を返す)
    - `mailbox_config.rs:137-141` (L140 で `ControlAwareRequiresUnboundedPolicy` を返す)
  - 当初の proposal は後者を見落としており、新 Bounded+ControlAware 分岐が validate で弾かれ unreachable dead code になる設計ミスを内包していた
- **選択**: **両 variant + 両 validate 分岐**を削除する。関連する rustdoc / tests / 他 caller もまとめて追随。
- **Rationale**:
  - `bounded + deque` / `bounded + control_aware` の両方が valid な組合せになるため、validate での拒否は論理的に不整合
  - 両 variant はいずれも「組合せを許容していない」意味なので残すと誤情報化する
  - CLAUDE.md 「後方互換は不要 / 破壊的変更を恐れずに最適な設計を追求」方針で variant 削除を許容
- **影響範囲** (rtk grep 実測):
  - `BoundedWithDeque`: 9 参照 / 6 ファイル
  - `ControlAwareRequiresUnboundedPolicy`: 5 参照 / 3 ファイル
- **代替**: `#[deprecated]` で残す案は却下 (pre-release なので意味がない)

### Decision 5: `create_message_queue_from_config` の control-aware 分岐に capacity 判定を追加する

- **選択**: 既存 `deque_mailbox_type_from_policy` / `priority_mailbox_type_from_config` / `stable_priority_mailbox_type_from_config` と同形の helper `control_aware_mailbox_type_from_policy(policy: MailboxPolicy) -> Box<dyn MailboxType>` を新設し、`Bounded { capacity }` → `BoundedControlAwareMailboxType::new(capacity, overflow)` / `Unbounded` → `UnboundedControlAwareMailboxType::new()` を dispatch する。`create_message_queue_from_config` の control-aware 枝はこの helper を呼ぶだけにする。
- **Rationale**:
  - 既存の他 3 helper (deque / priority / stable_priority) が `_mailbox_type_from_{policy,config}` 命名で揃っており、pattern 統一で可読性が上がる
  - Decision 4 で `ControlAwareRequiresUnboundedPolicy` を削除したため、validate を通過して dispatch に到達する bounded + control_aware 経路が新たに発生する。この経路で `policy.capacity()` を分岐せず無条件 Unbounded 生成すると bounded 指定が silently ignored される (既存 dispatch の残存 bug)。本 change で validate を緩める以上、dispatch 側も capacity 分岐で整合を取る必要がある
- **代替**: helper を切り出さず `create_message_queue_from_config` の control-aware 枝に inline match を書く案もあるが、既存 3 helper の pattern から外れるため却下

## Risks / Trade-offs

### Risk 1: BREAKING: `MailboxConfigError` 2 variant 削除

- **影響**: public enum から 2 variant (`BoundedWithDeque`, `ControlAwareRequiresUnboundedPolicy`) が削除される。下流 (workspace 内) で `match` していた箇所はコンパイルエラーになる。
- **範囲** (rtk grep 実測):
  - `BoundedWithDeque`: 9 参照 / 6 ファイル
  - `ControlAwareRequiresUnboundedPolicy`: 5 参照 / 3 ファイル
  - 合計 14 参照。両方を参照する 3 ファイル (mailbox_config_error.rs / mailbox_config.rs / mailbox_config/tests.rs) を差し引いた unique 対象は **6 ファイル**。外部クレートへの波及はなし (pre-release)
- **緩和**: CLAUDE.md 方針に従い破壊的変更を許容。tasks の caller 追随と `rtk grep "BoundedWithDeque|ControlAwareRequiresUnboundedPolicy"` で残参照ゼロ検証 + CI 全通しで担保

### Risk 2: control-aware + bounded の受理経路修正による挙動変化

- **従来の実際の挙動** (ultrareview で判明): `MailboxConfig::validate()` が `ControlAwareRequiresUnboundedPolicy` で **fail-fast** で拒否する。当初の proposal 記述 "silently unbounded fallback" は誤認で、`mailboxes.rs:54-57` の無条件 Unbounded 生成は validate を経由しない経路でのみ到達する
- **本 change 後**: validate は `Ok(())` を返し、`create_message_queue_from_config` が `BoundedControlAwareMessageQueue` を返す
- **影響**: 従来 `ControlAwareRequiresUnboundedPolicy` 前提で組まれていた caller は validate 成功経路に切替わる。`match` で variant を handle していたコードは 2 variant 削除により要修正
- **範囲**: workspace 内 14 参照すべてを tasks Phase 5 で列挙済 (Risk 1 と同じ)
- **緩和**: change log に BREAKING として明記、tasks と CI で網羅確認

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

- **影響**: push_front が容量超過した際の挙動が `DequeMessageQueue::enqueue_first` の契約と整合しない可能性。
- **緩和**: Decision 2-c に従い、`enqueue_first` は capacity チェックを行う:
  - Grow: 容量無視で push_front 成功
  - DropNewest / DropOldest: 容量超過なら `Err(SendError::Full(..))` (evict せず Reject)
  戻り値型 `Result<(), SendError>` と整合し、enqueue 経路 (EnqueueOutcome) との責務分離が保たれる。
