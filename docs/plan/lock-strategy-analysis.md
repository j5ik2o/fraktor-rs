# ロック戦略の全体整理と優先順位検討

**作成日**: 2026-04-08
**ステータス**: 調査・検討中（コード変更なし）
**関連 openspec change**: `openspec/changes/lock-driver-port-adapter/`
**関連 PR**: #1530 (utils-sync-collapse), #1535, #1537, #1538

## 背景

`DebugSpinSyncMutex` による deadlock 検知を実用化する過程で、単純な `RuntimeMutex` の Port/Adapter 化では済まない複数の設計課題が浮上した。本ドキュメントは、浮上した課題を整理し、優先順位と依存関係を視覚化して、次のアクションを決めるための意思決定材料を残す。

## 浮上している 3 つの課題

### 課題 1: ロック機構そのものの選定

現在の fraktor-rs は `utils-core` の `SpinSyncMutex<T>` (= `spin::Mutex<T>` の薄いラッパー) を `RuntimeMutex<T>` として全面採用している。これは **no_std 環境では妥当** だが、実際の caller 環境ごとに適切性が異なる:

```
┌──────────────────────────────────────────────────────────────┐
│ 環境 / 選択肢と特性                                          │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  no_std / bare-metal / embedded                              │
│  ├─ 選択肢が spin しかない (OS なし → park できない)         │
│  └─ SpinMutex が妥当 ✓                                       │
│                                                              │
│  std thread (OS スレッドで直接実行)                          │
│  ├─ std::sync::Mutex     : futex/SRWLock, 可                 │
│  ├─ parking_lot::Mutex   : より速い, well-tested, 推奨       │
│  └─ SpinMutex            : 短いクリティカルセクションなら可  │
│                             だが park しないので contention  │
│                             下で CPU 100% を浪費             │
│                                                              │
│  tokio executor thread (async worker)                        │
│  ├─ std::sync::Mutex     : 短時間ならOK                      │
│  │                         **但し .await を跨いではならない**│
│  │                         worker thread を park させるので  │
│  │                         並列度が落ちる (lock 時間次第)    │
│  ├─ parking_lot::Mutex   : 同上、速い                        │
│  ├─ tokio::sync::Mutex   : .await を跨ぐ時専用、             │
│  │                         sync 用途ではオーバーヘッド過剰   │
│  └─ SpinMutex            : 絶対にダメ → 同じ worker 上で     │
│                             lock 待ちがスケジュール不能に    │
│                             なると deadlock (starvation)     │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

特に **tokio worker 上での SpinMutex が危険** な理由:

```
Worker A (持ち主)              Worker B (待ち側)
────────────────              ────────────────
lock OK                       lock() → spin
...処理中...                   spin 中 (CPU 100%)
                              spin 中 (CPU 100%)  ← Worker B の全時間を消費
yield する隙なし               ← yield_now() されないので
(A は別タスクを                   tokio scheduler は B に別タスクを
 走らせられない)                   アサインできない
────────────────              ────────────────
```

`tokio::runtime::Builder::new_current_thread()` の single-threaded runtime では、**1 本の worker 上で自己 deadlock** する可能性もある (SpinMutex の再入不可 + async が混ざった時)。

fraktor-rs の dispatcher は tokio runtime 上で動くケースが中心 (actor-adaptor-std 経由) で、この危険性を直接抱えている。

### 課題 2: Mailbox の二重ロック

`modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` を調査した結果、Mailbox が **二重にロックを取っている** ことが判明した:

```rust
// 外側: Mailbox 本体
pub struct Mailbox {
  user_queue_lock: ArcShared<RuntimeMutex<()>>,  // ← ① unit 型の barrier lock
  user:            Box<dyn MessageQueue>,         // ← ② 内部でも RuntimeMutex 保持
  // ...
}

// enqueue_envelope の実装
pub fn enqueue_envelope(&self, envelope: Envelope) -> Result<(), SendError> {
  if self.is_suspended() { return Err(...); }
  let enqueue_result = {
    let _guard = self.user_queue_lock.lock();   // ← ① 外側ロック獲得
    self.user.enqueue(envelope)                 // ← ② 内部で更にロック獲得
  };
  // ...
}
```

一方、`unbounded_deque_message_queue.rs` の内部実装:

```rust
pub struct UnboundedDequeMessageQueue {
  inner: RuntimeMutex<VecDeque<Envelope>>,  // ← ② 内部 mutex
}

impl MessageQueue for UnboundedDequeMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<(), SendError> {
    // ② を取って push
  }
}
```

つまり **通常の単一 enqueue が 2 段のロックを通過する** 構造になっている。

#### 外側ロックの必要性 (仮説)

`prepend_user_messages` の実装から推測すると、外側 `user_queue_lock` は compound op (`number_of_messages() → overflow check → enqueue`) を atomic 化するために存在する:

```rust
pub(crate) fn prepend_user_messages(&self, messages: &VecDeque<AnyMessage>) -> Result<(), SendError> {
  // ...
  let _guard = self.user_queue_lock.lock();                         // ← ① 外側を取る
  let current_user_len = self.user.number_of_messages();            // ② 内部で mutex 取得
  if self.prepend_would_overflow(messages.len(), current_user_len) {
    return Err(SendError::full(first_message));
  }
  if let Some(deque) = self.user.as_deque() {
    return self.prepend_via_deque(deque, messages);                 // ② で複数回 enqueue
  }
  self.prepend_via_drain_and_requeue(messages, &first_message)
}
```

これは Pekko の `BoundedMailbox.putLock: ReentrantLock` と責務がほぼ同じと思われる (put の atomicity 保証)。ただし Pekko は **put にだけ** 使っており、通常の enqueue は lock-free atomic で済ませている。

#### 考えられる解決策

| 選択肢 | 実現方法 | 問題 |
|---|---|---|
| **A** | `MessageQueue` trait に `try_enqueue_with_capacity_check` 等の compound op を追加し、各 impl 内で atomic 化 | trait API が太る、各 impl で内部 mutex を重複的に扱う |
| **B** | `user_queue_lock` を `put_lock: RuntimeMutex<()>` に改名し、**prepend/bounded capacity check の時だけ** 取る。通常 enqueue では取らない | Pekko 相当、最小変更、要再設計 |
| **C** | 外側と内側を統合: Mailbox 本体で `RuntimeMutex<MessageQueue>` 相当を持ち、internal queue 型には mutex を持たせない | 大規模リファクタ |
| **D** | lock-free MPSC queue (crossbeam / heapless) に切り替えて両方撤廃 | no_std 制約の検証必要、lock-free は複雑 |

### 課題 3: LockDriver Port/Adapter

元々 `DebugSpinSyncMutex` による deadlock 検知を実用化するために提案した change (`openspec/changes/lock-driver-port-adapter/`)。

- `utils-core` に `LockDriver<T>` / `RwLockDriver<T>` trait を新設
- `RuntimeMutex<T, D>` を struct 化 (**デフォルト型引数なし**)
- `LockDriverFactory` で多 T フィールドのジェネリック化
- actor-core/kernel を `<F: LockDriverFactory>` でジェネリック化
- test 時に `DebugSpinSyncFactory` を差し替え可能にする

この change 自体は (1) と (2) の **測定・検証インフラ** としても機能する。driver を swap できれば、parking_lot vs spin の実測比較や、lock-free queue への段階的移行の土台になる。

## 参照実装 (Pekko / protoactor-go) との対比

| 参照実装 | Mailbox 同期戦略 |
|---|---|
| **protoactor-go** | `sync.Mutex` **ゼロ**。`atomic.CompareAndSwap` + lock-free MPSC queue のみ |
| **Apache Pekko** | `AtomicReferenceFieldUpdater` + lock-free queue。`ReentrantLock` は bounded mailbox の `putLock` のみ |
| **fraktor-rs 現状** | `user_queue_lock: RuntimeMutex<()>` + 内部 queue の `RuntimeMutex<VecDeque>` の **二重ロック** (SpinMutex) |

### protoactor-go `defaultMailbox.PostUserMessage` の構造

```go
m.userMailbox.Push(message)           // ← MPSC queue (lock-free)
atomic.AddInt32(&m.userMessages, 1)   // ← atomic
m.schedule()                          // ← atomic CAS (idle → running)
```

**total: 0 locks per enqueue**

### Pekko `Mailbox` の関連部分 (抜粋)

```scala
// AtomicReferenceFieldUpdater for system queue.
private final val size = new AtomicInteger(0)
private final val putLock = new ReentrantLock()  // bounded put path のみ
```

通常の enqueue 路は atomic only。`putLock` は bounded mailbox で put を atomic 化するため。

### fraktor-rs `Mailbox.enqueue_envelope`

```rust
let _guard = self.user_queue_lock.lock();   // ← 外側 spin lock (unit type)
self.user.enqueue(envelope)                 // ← 内部で RuntimeMutex<VecDeque> も spin lock
```

**total: 2 locks per enqueue**

参照実装と比較すると、fraktor-rs の現状は **最適解から 2 段階ほど離れている**:

1. ロックの段数が不必要に多い (2 段)
2. ロック実装が tokio 環境で危険な spin になっている

## 依存関係と優先順位の視覚化

```
                    ┌──────────────────────────────────────┐
                    │  (1) ロック機構そのものの選定        │
                    │  spin / parking_lot / std::sync /    │
                    │  tokio::sync / lock-free             │
                    └──────────────────────────────────────┘
                         ▲              ▲               ▲
                         │ swap可能化   │ 測定可能化    │ 不要化
                         │              │               │
                    ┌────┴──────┐  ┌────┴────┐   ┌──────┴───────┐
                    │ (3) Port  │  │ ベンチ  │   │ (2) 二重ロック│
                    │  Adapter  │  │ マーク  │   │   解消        │
                    │  LockDrv  │  │         │   │               │
                    └───────────┘  └─────────┘   └───────────────┘
                         │                             │
                         │ test 差し替え                │ compound op の
                         │ deadlock 検知                │ 原子化が必要
                         ▼                             ▼
                    ┌───────────────┐        ┌──────────────────┐
                    │ 当面の主目的  │        │ MessageQueue     │
                    │ (PR #1538 で  │        │ trait の拡張 or  │
                    │  宙吊り)      │        │ lock-free queue  │
                    └───────────────┘        └──────────────────┘
```

### 観察

- **(3) Port/Adapter は (1) と (2) の両方の「測定・検証インフラ」になる** — driver を swap できれば実測データが取れる
- **(2) 二重ロックは (3) と独立に修正可能** — `MessageQueue` trait を拡張すれば外側の `user_queue_lock: RuntimeMutex<()>` を撤廃できる可能性
- **(1) は本当は (2) と (3) の結果を見てから判断すべきデータドリブンな選択** — 現時点で理論だけで決めると後悔する
- **(2) + (1-lock-free) の組み合わせが究極の最適解** だが、no_std + alloc 制約下で lock-free queue を成立させられるか要調査

## 進行順序 4 案

### 案 α: Port/Adapter → 二重ロック → 選定

```
期間1: (3) Port/Adapter (LockDriver, 115 files)
 └─ test-time DebugSpinSync で deadlock 検知可能に
期間2: (2) 二重ロック調査 + 修正
 └─ user_queue_lock の必要性判定、削れるなら削る
期間3+: ベンチ → (1) 選定 (parking_lot? lock-free?)
```

**利点**:
- 現在進行中の openspec change を活かせる
- deadlock 検知という主目的を早く達成
- 測定インフラが先にできる

**欠点**:
- 115 file の migration を先に被る
- 二重ロックが削れることが後で分かったら、Port 対象の一部 (`user_queue_lock: RuntimeMutex<()>`) は無駄だった
- (1) の判断がまだなのに caller boundary で `SpinSyncFactory` 固定する必要がある

### 案 β: 二重ロック → Port/Adapter → 選定

```
期間1: (2) 二重ロック調査 + 修正 (user_queue_lock 撤廃)
 └─ MessageQueue trait に compound op 追加、または
    既存の内部 mutex を使い compound op を再設計
期間2: (3) Port/Adapter (対象 caller が減った状態で)
期間3+: (1) 選定
```

**利点**:
- Port 対象の caller 数が減る (二重ロックが先に消えるので)
- 二重ロック削減は Port/Adapter に依存しない純粋な改善
- 最小のインクリメンタル改善

**欠点**:
- deadlock 検知が後回し (PR #1538 の宙吊り状態が続く)
- `MessageQueue` trait の拡張が簡単ではないかもしれない (全 impl の更新)

### 案 γ: 調査先行 → Port → 選定 (本ドキュメント自身)

```
期間1: docs/plan/lock-strategy-analysis.md (本ドキュメント) を書く
 ├─ 二重ロックの根拠を読解
 ├─ compound op の atomic 化の難易度を評価
 ├─ 参照実装 (pekko / protoactor-go) の lock-free 戦略を検証
 ├─ no_std + lock-free queue の実現可能性 (heapless?)
 └─ 決定: (2) が解けるか、(1) の default をどうするか
期間2: 決定された方針で (3) Port/Adapter + (2) 修正
期間3+: (1) 選定
```

**利点**:
- 最小のリスク、理解を深めてから実装
- Port/Adapter の設計が調査結果に応じて調整できる
- 後の方向転換コストが低い

**欠点**:
- 進捗が見えにくい (調査フェーズ)
- deadlock 検知の実現が遅れる

### 案 δ: lock-free 全振り

```
期間1: Mailbox を lock-free 化 (crossbeam 相当 or 自作 MPSC)
 └─ user_queue_lock 撤廃、内部 queue も lock-free に
期間2+: 残る RuntimeMutex 用途について (1) 選定
```

**利点**:
- 参照実装 (protoactor-go) と整合する最適解
- 二重ロックも lock 選定も迂回 (Mailbox については)

**欠点**:
- no_std 制約 (crossbeam は std only)、代替ライブラリ (heapless, bbqueue) の調査必要
- lock-free データ構造のデバッグは hard mode
- Mailbox 以外の shared state (`ActorCell`, `SystemState`, etc.) では依然ロックが必要
- スコープが巨大

## 推奨: **案 γ (調査先行)**

### 理由

1. **今得ている情報で決めるには情報不足**。特に:
   - 二重ロックが本当に必要か (compound op の列挙と atomicity 要件の検証) → 30分-数時間の読解で判明する
   - Pekko の `putLock` が ReentrantLock を使う理由と、fraktor-rs の `user_queue_lock` が同じ責務を持つかどうか
   - `MessageQueue` trait を拡張して compound op を atomic 化できるか
   - no_std + lock-free queue の実現可能性 (heapless::mpmc::Q64 などの調査)
2. **調査コストが極めて低い** (コード読むだけ)。成果物は本ドキュメントの延長で、直接レビュー可能
3. **調査結果が出たら α/β/δ のどれに進むか自然に決まる**:
   - 二重ロック削れる → β (まず削って Port 対象減らす)
   - 二重ロック削れない → α (Port を先にして測定インフラ整える)
   - lock-free が no_std で成立する → δ を検討
   - どちらも難しい → α + (1) を後日
4. **現在の openspec change (`lock-driver-port-adapter`) は捨てずに保留できる** — 調査後に proposal を微調整するだけ

## 調査結果

### A. 二重ロックの必要性判定

#### A.1 `user_queue_lock` の全使用箇所 (7 箇所)

`modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` をソース読解した結果、`user_queue_lock: ArcShared<RuntimeMutex<()>>` は以下の 7 箇所で acquire されている:

| # | 行 | メソッド | 操作 | compound op? |
|---|---|---|---|---|
| 1 | 320-322 | `enqueue_envelope` | 単一 enqueue | **いいえ** (単純 put) |
| 2 | 352-364 | `prepend_user_messages` | check + multi enqueue | **はい** (check + put 多段) |
| 3 | 390-432 | `prepend_via_drain_and_requeue` (#2 の延長) | drain all → enqueue new → restore old | **はい** (最も複雑) |
| 4 | 448-450 | `dequeue` | 単一 dequeue | **いいえ** (単純 pop) |
| 5 | 522-528 | `become_closed_and_clean_up` | drain all to dead letters | **はい** (shutdown 時のみ) |
| 6 | 548-549 | `user_len` | `number_of_messages()` 1 回 | **いいえ** (単純 read) |
| 7 | 578-580 | `publish_metrics` | `number_of_messages()` 1 回 | **いいえ** (単純 read) |

**観察**: 7 箇所のうち、compound op は 3 箇所 (#2, #3, #5) のみ。残る 4 箇所 (#1, #4, #6, #7) は **単純 put/pop/read** であり、外側ロックは本来不要。

#### A.2 実際に走るロックの段数 (BoundedMessageQueue の場合)

`enqueue_envelope` で `BoundedMessageQueue` を使う caller が enqueue するとき、実際に取られる mutex をトレースした結果、**3 段のネストロック** になっていた:

```
Mailbox::enqueue_envelope(envelope)
│
├─ [LOCK 1] self.user_queue_lock.lock()              ← RuntimeMutex<()>
│                                                       (Mailbox 外側の barrier lock)
│
├─ self.user.enqueue(envelope)
│  │
│  └─ BoundedMessageQueue::enqueue
│     │
│     └─ offer_if_room (DropNewest の場合)
│        │
│        └─ QueueStateHandle::offer_if_room
│           │
│           ├─ [LOCK 2] self.state.lock()            ← RuntimeMutex<QueueState<T>>
│           │                                          (QueueState の整合性用)
│           │
│           ├─ state.len() >= capacity check
│           │
│           └─ state.offer(message)
│              │
│              └─ QueueState::offer (&mut self)
│                 │
│                 └─ self.queue.offer(message)
│                    │
│                    └─ UserQueueShared::offer
│                       = SyncQueueShared<T, VecDequeBackend<T>>::offer
│                          │
│                          └─ [LOCK 3] self.inner.lock()  ← SpinSyncMutex<SyncQueue<T, B>>
│                                                              (backend 実体用)
│
└─ 戻る
```

**事実**: `BoundedMessageQueue` 経路の 1 回の enqueue で **3 段の SpinSyncMutex を順次取得している**。うち少なくとも LOCK 1 と LOCK 3 は代替手段なしに同時に取得される。LOCK 2 も `QueueState` 内の `AtomicUsize size` と `WaitQueue` を一緒に扱うために保持されている。

#### A.3 `UnboundedDequeMessageQueue` の場合

```
Mailbox::enqueue_envelope(envelope)
│
├─ [LOCK 1] self.user_queue_lock.lock()              ← RuntimeMutex<()>
│
└─ self.user.enqueue(envelope)
   │
   └─ UnboundedDequeMessageQueue::enqueue
      │
      └─ [LOCK 2] self.inner.lock()                  ← RuntimeMutex<VecDeque<Envelope>>
```

**2 段** (LOCK 1 + LOCK 2)。`VecDeque::push_back` が 1 回走るだけの単純 op に 2 段の spin lock が重なっている。

#### A.4 compound op のためのロック必要性検討

3 つの compound op (prepend、drain_and_requeue、close_and_clean_up) について、外側ロックを削除可能かを個別に検証:

##### compound op (2): `prepend_user_messages` (via deque)

```rust
let _guard = self.user_queue_lock.lock();
let current_user_len = self.user.number_of_messages();
if self.prepend_would_overflow(messages.len(), current_user_len) {
  return Err(SendError::full(first_message));
}
if let Some(deque) = self.user.as_deque() {
  return self.prepend_via_deque(deque, messages);  // → enqueue_first を複数回
}
```

**必要な atomicity**: `number_of_messages` 読み取り → capacity check → `enqueue_first` を N 回。
途中で他 producer が enqueue すると overflow check が壊れる。

**解決案**:
- **案 a1**: `MessageQueue` trait に `prepend_batch_with_capacity(msgs: &[Envelope], capacity_limit: usize) -> Result<(), SendError>` を追加し、各 impl が内部 mutex で atomic 化する → **trait API が太るが実現可能**
- **案 a2**: 外側 `put_lock: RuntimeMutex<()>` を **prepend / drain_and_requeue / close 時のみ** 取る形に再設計し、通常 enqueue/dequeue は取らない (Pekko の `putLock` 相当) → **最小変更で実現可能**

##### compound op (3): `prepend_via_drain_and_requeue`

```rust
// drain all
while let Some(envelope) = self.user.dequeue() { existing.push_back(envelope); }
// enqueue new + existing
for envelope in new_envelopes.chain(existing_envelopes) {
  self.user.enqueue(envelope);
}
// error recovery: re-enqueue existing
```

**必要な atomicity**: drain → rebuild → error recovery の全体を atomic に。
途中で他 producer が enqueue すると、順序がぐちゃぐちゃになる。

**解決案**:
- 案 a1 と同じ trait 拡張 (`prepend_batch_with_capacity`) で内部 atomic 化 → **各 impl で lock を maintain すれば可能**
- 案 a2 の put_lock を維持 → **可能**

##### compound op (5): `become_closed_and_clean_up`

```rust
self.state.close();   // state transition (lock-free)
...
let _guard = self.user_queue_lock.lock();
while let Some(envelope) = self.user.dequeue() { ... }   // drain all
self.user.clean_up();
```

**必要な atomicity**: `state.close()` 後の drain を他 enqueue と混じらせない。
ただし `state.close()` 後は他 producer の enqueue も suspended になるので、実質的に producer はいない。

**解決案**:
- **close 後の barrier は state transition 側 (`MailboxScheduleState`) で強制可能**。close 後に enqueue_envelope が呼ばれても `is_suspended` で弾かれる
- つまり、**厳密には外側ロックは不要** だが、安全側に倒すなら残す
- 残すとしても、close 経路は 1 回しか走らないので hot path 影響なし

#### A.5 単純 op の不要ロック削減検討

4 つの単純 op (#1, #4, #6, #7) について、**外側ロックを外しても安全か** を検証:

##### op (1): `enqueue_envelope` の単一 enqueue

- 内側の `user.enqueue` は既に atomic (各 impl が内部 mutex を持つ or QueueStateHandle 経由で atomic)
- 外側 `user_queue_lock` を取る理由は **compound op (2/3) と同時に走ることを防ぐため**
- 外側ロックを外すと、(2/3) の途中で enqueue が割り込んで順序が壊れる
- → **(2/3) と排他が必要なだけで、純粋な enqueue には不要**
- → 案 a2 (put_lock を compound op 専用にする) なら、通常 enqueue は **lock 不要**

##### op (4): `dequeue`

- consumer は通常 1 つだけ (dispatcher 側のループ)。producer と dequeue が同時に走っても、各 impl の内部 mutex で衝突は解消される
- ただし compound op (2/3) が途中で走るとおかしくなる
- → **(2/3) と排他が必要なだけ**
- → 案 a2 で改善可能

##### op (6): `user_len` / op (7): `publish_metrics` の `number_of_messages()`

- 単純 read op
- 内側 mutex で取得可能な値 (bounded の場合は `QueueState.size: AtomicUsize`)
- 外側ロックを外すと、compound op の **途中の中間値** が見えるリスク
- しかし metric 目的なら中間値でも致命的ではない (ちょっと不正確でよい)
- → **外側ロック不要**

#### A.6 結論: 二重ロック (実は三重ロック) は削減可能

ソース読解の結果:

1. **実態は「三重ロック」** (BoundedMessageQueue 経路)。「二重」と見えていたのは過小評価だった
2. **compound op のうち close は実は不要** (state 側で suspend 強制される)
3. **通常 enqueue/dequeue の外側ロックは compound op との排他のためだけ** に存在
4. 以下の 2 案のいずれかで削減可能:
   - **案 a1**: `MessageQueue` trait に `prepend_batch_with_capacity` / `drain_and_rebuild_atomic` を追加。各 impl が内部 mutex で atomic 化。外側 `user_queue_lock` 撤廃
   - **案 a2**: 外側 `user_queue_lock` を `put_lock: RuntimeMutex<()>` に改名し、**prepend と drain_and_requeue の時だけ取る**。通常 enqueue/dequeue/read は取らない (Pekko の `putLock` 方式)

**実装コストと副作用**:

| 案 | trait 変更 | impl 変更 | 外側ロック | 通常 enqueue の段数 (Bounded) | 通常 enqueue の段数 (Unbounded) |
|---|---|---|---|---|---|
| 現状 | — | — | 常時保持 | **3 段** | **2 段** |
| a1 | `prepend_batch_with_capacity` 追加 | 全 impl で新 API 実装 | **撤廃** | 2 段 (QueueState + SyncQueueShared) | 1 段 (VecDeque inner のみ) |
| a2 | 変更なし | 変更なし | compound op 時のみ | 2 段 (内側 2 段のみ) | 1 段 (内側 1 段のみ) |

**推奨**: **案 a2 を先に採用** (最小変更で段数を減らせる)。その後、必要なら案 a1 に発展させる。

さらに、`QueueStateHandle` 側の LOCK 2 と `SyncQueueShared` 側の LOCK 3 の **内側二重ロック** は別問題として残る。これは:

- LOCK 2: `QueueState` の整合性 (`queue` と `size: AtomicUsize` と `WaitQueue`) 用
- LOCK 3: backend の整合性用

LOCK 2 と LOCK 3 を統合するには `QueueState` の設計を変える必要があり、本 change のスコープを大きく超える。**独立した別 change 候補** として記録する。

### C. Pekko / protoactor-go lock-free 戦略との対比

#### C.1 Pekko の Mailbox 種別と lock 使用状況

`references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Mailbox.scala` を読解した結果、Pekko は **Mailbox の種類ごとに異なる queue 実装** を選ぶ設計になっている:

| Mailbox 種別 | Queue 実装 | 外部 putLock | ロック種別 | 備考 |
|---|---|---|---|---|
| `UnboundedMailbox` (**default**) | `java.util.concurrent.ConcurrentLinkedQueue` | **なし** | lock-free (Michael-Scott queue) | デフォルトは完全 lock-free |
| `SingleConsumerOnlyUnboundedMailbox` | `NodeMessageQueue` (`AbstractNodeQueue`) | **なし** | lock-free (Vyukov MPSC) | high-performance MPSC |
| `NonBlockingBoundedMailbox` | `BoundedNodeMessageQueue` | **なし** | lock-free bounded MPSC | bounded + lock-free |
| `BoundedMailbox` | `java.util.concurrent.LinkedBlockingQueue` | なし (queue 内蔵の putLock/takeLock のみ) | blocking 内蔵 | JDK 標準 blocking queue |
| `BoundedControlAwareMailbox` | `ConcurrentLinkedQueue` + `controlQueue` | **あり** (`ReentrantLock putLock`) | lock-free + timeout 用 cond var | 制御メッセージ優先 + `awaitNanos` |

**観察**:
- **Pekko の default (UnboundedMailbox) は 100% lock-free**。`ConcurrentLinkedQueue` を直接継承しており、Mailbox 層での追加ロックはない
- **`ReentrantLock putLock` が登場するのは `BoundedControlAwareMailbox` のみ**。その用途は:
  1. capacity 到達時の timeout-blocking enqueue (`notFull.awaitNanos`)
  2. consumer dequeue 後の `notFull` 通知 (Condition variable signal)
  3. control queue の優先順位保証
- **通常の Bounded (`BoundedMailbox`) は `LinkedBlockingQueue` に任せきり**。putLock/takeLock は JDK の BlockingQueue 実装内部に隠蔽され、Mailbox 層では見えない

#### C.2 Pekko `NodeMessageQueue` / `AbstractNodeQueue` の実装

`references/pekko/actor/src/main/java/org/apache/pekko/dispatch/AbstractNodeQueue.java` から:

```java
/**
 * Lock-free MPSC linked queue implementation based on Dmitriy Vyukov's non-intrusive MPSC queue:
 * https://www.1024cores.net/home/lock-free-algorithms/queues/non-intrusive-mpsc-node-based-queue
 */
public abstract class AbstractNodeQueue<T> extends AtomicReference<AbstractNodeQueue.Node<T>> {
    // head slot = AtomicReference<Node<T>> (from superclass)
    private volatile Node<T> _tailDoNotCallMeDirectly;
    // ...
}
```

- **AtomicReference for head**: producer が CAS で head を更新
- **VarHandle for tail**: consumer が tail を進める (single consumer なので CAS 不要)
- **Vyukov の非侵入型 MPSC queue アルゴリズムを採用**

#### C.3 protoactor-go の Mailbox

`references/protoactor-go/actor/mailbox.go` から:

```go
type defaultMailbox struct {
    userMailbox     queue              // ← lock-free MPSC queue
    systemMailbox   *mpsc.Queue        // ← lock-free MPSC queue (internal/queue/mpsc)
    schedulerStatus int32              // ← atomic
    userMessages    int32              // ← atomic
    sysMessages     int32              // ← atomic
    suspended       int32              // ← atomic
    invoker         MessageInvoker     // ← 非同期 updater 経由 (単発 assignment)
    // ...
}

func (m *defaultMailbox) PostUserMessage(message interface{}) {
    // middleware 呼び出し
    m.userMailbox.Push(message)              // ← lock-free push
    atomic.AddInt32(&m.userMessages, 1)      // ← atomic increment
    m.schedule()                              // ← atomic CAS (idle → running)
}
```

- **`sync.Mutex` 使用ゼロ** (hot path)
- `sync.Mutex` は `actor/future.go` の `sync.Cond` 用と、test mock にしか登場しない
- 通常 enqueue は **2 つの atomic op (Push 内の atomic + size counter の AddInt32)** だけで完了

#### C.4 `references/protoactor-go/internal/queue/mpsc/mpsc.go` の実装

```go
// Vyukov 非侵入型 MPSC queue
type Queue struct {
    head, tail *node
}

func (q *Queue) Push(x interface{}) {
    n := new(node)
    n.val = x
    prev := (*node)(atomic.SwapPointer((*unsafe.Pointer)(unsafe.Pointer(&q.head)), unsafe.Pointer(n)))
    atomic.StorePointer((*unsafe.Pointer)(unsafe.Pointer(&prev.next)), unsafe.Pointer(n))
}

func (q *Queue) Pop() interface{} {
    tail := q.tail
    next := (*node)(atomic.LoadPointer((*unsafe.Pointer)(unsafe.Pointer(&tail.next))))
    if next != nil {
        q.tail = next
        v := next.val
        next.val = nil
        return v
    }
    return nil
}
```

- **2 つの atomic op** (SwapPointer + StorePointer) で push 完了
- **1 つの atomic op** (LoadPointer) で pop 完了
- Pekko の `AbstractNodeQueue` と **同じアルゴリズム** (両方とも Vyukov の reference 実装を引用)

#### C.5 fraktor-rs との対比

| 実装 | default Mailbox の通常 enqueue | lock 段数 | atomic op 数 |
|---|---|---|---|
| **Pekko** `UnboundedMailbox` | `ConcurrentLinkedQueue.offer` | 0 段 | O(1) CAS |
| **protoactor-go** `defaultMailbox.PostUserMessage` | `mpsc.Queue.Push` + atomic counter | 0 段 | 2-3 atomic op |
| **fraktor-rs** `Mailbox::enqueue_envelope` (UnboundedDeque) | `user_queue_lock.lock()` → `UnboundedDequeMessageQueue::enqueue` | **2 段** | 0 atomic op (全 spin lock) |
| **fraktor-rs** `Mailbox::enqueue_envelope` (Bounded) | `user_queue_lock.lock()` → `QueueStateHandle::offer_if_room` → `SyncQueueShared::offer` | **3 段** | 0 atomic op (全 spin lock) |

**結論**: fraktor-rs は参照実装と **本質的に異なるロック戦略** を採用している。参照実装は lock-free queue ベースなのに対し、fraktor-rs は lock-based queue ベース。それぞれの trade-off:

| 項目 | lock-free queue (Pekko/protoactor-go) | lock-based queue (fraktor-rs 現状) |
|---|---|---|
| hot path の atomic op 数 | 2-3 | 0 (全て lock) |
| hot path の lock 段数 | 0 | 2-3 段 |
| contention 時の挙動 | CAS retry loop | spin (SpinMutex の場合 CPU 100%) |
| 実装の複雑さ | 高 (lock-free アルゴリズム) | 低 (mutex + 標準データ構造) |
| デバッグ容易性 | 低 | 高 (stack trace で状態見える) |
| no_std 可否 | 自作する必要あり | `spin::Mutex` で素直 |
| tokio worker 上での安全性 | 安全 (待ちが発生しない) | **不安定** (spin が worker を占有) |
| メモリアロケーション | 各 push で node 割当 | backend 依存 (VecDeque なら amortized) |

#### C.6 Pekko `putLock` が「通常 enqueue に無い」ことの意味

fraktor-rs の `user_queue_lock` を Pekko の `ReentrantLock putLock` と比較すると、**責務が似ているが適用範囲が違う**:

| 項目 | Pekko `putLock` (BoundedControlAwareMailbox) | fraktor-rs `user_queue_lock` |
|---|---|---|
| 使用箇所 | `BoundedControlAwareMailbox` のみ (default ではない) | **全 Mailbox 種別 (default 含む)** |
| 通常 enqueue で取るか | いいえ (lock-free queue 側で処理) | **はい** (hot path で常時取得) |
| 主目的 | `awaitNanos` timeout + `notFull.signal()` | compound op (prepend/drain) atomicity |
| 代替手段 | lock-free bounded queue で不要 | trait API 拡張 or put_lock 限定化で不要 |

**fraktor-rs は Pekko の特殊ケース (`BoundedControlAwareMailbox`) の設計を default に一般化してしまっている** とも読める。これは参照実装と照らすと過剰であり、改善余地がある。

#### C.7 結論: 参照実装準拠の進路

参照実装と整合させるなら、以下の 2 段階が理想:

1. **第 1 段階 (案 a2 相当)**: `user_queue_lock` を `put_lock` に改名し、**prepend/drain/close 時のみ取る**。通常 enqueue/dequeue/read は外側ロックなし。Pekko の `BoundedControlAwareMailbox putLock` に準拠
2. **第 2 段階 (lock-free 化)**: `UnboundedDequeMessageQueue` / `BoundedMessageQueue` の内部実装を Vyukov MPSC queue ベースに差し替え。`SpinSyncMutex` 使用を撲滅

第 1 段階は trait 変更なしで実現可能で、影響範囲も狭い。第 2 段階は lock-free queue の自作 (または既存 crate 採用) を要し、大きな別 change になる。

### B. ロック使用箇所の contention 特性分類

`modules/` 配下 (`utils-core/src/core/sync/` と tests 配下を除く) で `RuntimeMutex` / `RuntimeRwLock` / `SpinSyncMutex` / `SpinSyncRwLock` を field 型として保持する全ての型を列挙し、contention 特性に基づいて分類した。合計 **50 箇所**。

#### B.1 per-actor (hot path) — メッセージディスパッチ

| crate | 型 | field | 対象 T | 種類 | 備考 |
|---|---|---|---|---|---|
| actor-core | Mailbox | user_queue_lock | `()` | Mutex | enqueue/dequeue 毎回、複数 sender |
| actor-core | Mailbox | invoker | `Option<MessageInvokerShared>` | Mutex | run 時メッセージ処理 |
| actor-core | Mailbox | actor | `Option<WeakShared<ActorCell>>` | Mutex | run 時セルチェック |
| actor-core | ActorCell | mailbox | `Option<ArcShared<Mailbox>>` | Mutex (Spin) | 一度設定後読み取り主体 |
| actor-core | ActorCell | state | `ActorCellState` | Mutex | 単一 dispatcher アクセス |
| actor-core | ActorCell | receive_timeout | `Option<ReceiveTimeoutState>` | Mutex | 単一 actor |
| stream-core | SinkQueue | inner | `SinkQueueInner<T>` | Mutex (Spin) | offer/consume 毎回 |
| stream-core | SourceQueue | inner | `SourceQueueState<T>` | Mutex (Spin) | offer/consume 毎回 |
| stream-core | BoundedSourceQueue | inner | `BoundedSourceQueueState<T>` | Mutex (Spin) | offer/consume 毎回 |
| stream-core | SourceQueueWithComplete | inner | `SourceQueueWithCompleteState<T>` | Mutex (Spin) | offer/complete 毎回 |

**合計 10 個** (actor-core 6, stream-core 4)。 **この 10 個が最重要改善対象**。

#### B.2 per-actor (non-hot path) — 計測・メトリクス

| crate | 型 | field | 対象 T | 種類 | 備考 |
|---|---|---|---|---|---|
| actor-core | Mailbox | instrumentation | `Option<MailboxInstrumentation>` | Mutex | メトリクス取得時のみ |

**合計 1 個**。

#### B.3 per-system (hot path) — イベント・スケジューリング

| crate | 型 | field | 対象 T | 種類 | 備考 |
|---|---|---|---|---|---|
| actor-core | EventStreamShared | inner | `EventStream` | **RwLock** | 全 actor 購読・発行 |
| actor-core | SchedulerShared | inner | `Scheduler` | **RwLock** | 全 actor タイマー登録・取得 |
| stream-core | PartitionHub | state | `PartitionHubState<T>` | Mutex (Spin) | 複数 producer/consumer |
| stream-core | MergeHub | state | `MergeHubState<T>` | Mutex (Spin) | 複数 producer/consumer |
| stream-core | BroadcastHub | subscribers | `Vec<VecDeque<T>>` | Mutex (Spin) | 複数 producer/consumer |

**合計 5 個** (actor-core 2, stream-core 3)。RwLock 採用で read 優位の contention は緩和されている。

#### B.4 per-system (non-hot path) — レジストリ・初期化・シャットダウン

| crate | 型 | field | 対象 T | 種類 | 備考 |
|---|---|---|---|---|---|
| actor-core | SystemStateShared | inner | `SystemState` | RwLock | actor lookup/register |
| actor-core | SerializationRegistry | serializers | `HashMap<..>` | RwLock | serialize lookup |
| actor-core | SerializationRegistry | bindings | `HashMap<..>` | RwLock | type→serializer mapping |
| actor-core | SerializationRegistry | manifest_routes | `HashMap<..>` | RwLock | manifest routing |
| actor-core | SerializationRegistry | cache | `HashMap<..>` | RwLock | cached lookup |
| actor-core | ActorRefProviderShared | inner | `ActorRefProviderHandle<P>` | Mutex | path lookup |
| actor-core | CellsShared | inner | `Cells` | Mutex | cell registry |
| actor-core | CircuitBreakerShared | inner | `CircuitBreaker<C>` | Mutex | failure state |
| actor-core | TerminationState | wakers | `Vec<Waker>` | Mutex | shutdown のみ |
| actor-core | CoordinatedShutdown | tasks | `BTreeMap<..>` | Mutex | startup/shutdown |
| actor-core | CoordinatedShutdown | reason | `Option<..>` | Mutex | shutdown reason |
| cluster-core | ClusterProviderShared | inner | `Box<dyn ClusterProvider>` | Mutex | cluster init |
| cluster-core | ClusterExtension | core | `ClusterCore` | Mutex | cluster state |
| cluster-core | ClusterExtension | subscription | `Option<..>` | Mutex | lifecycle |
| cluster-core | ClusterExtension | terminated | `bool` | Mutex | state flag |
| cluster-core | ClusterExtension | self_member_status | `Option<..>` | Mutex | membership |
| persistence-core | PersistenceExtensionShared | inner | `PersistenceExtension` | Mutex | init |

**合計 17 個**。

#### B.5 global / system-singleton — システムインフラ

| crate | 型 | field | 対象 T | 種類 | 備考 |
|---|---|---|---|---|---|
| actor-core | MessageDispatcherShared | inner | `Box<dyn MessageDispatcher>` | Mutex | dispatcher |
| actor-core | ExecutorShared | inner | `Box<dyn Executor>` | Mutex | executor |
| actor-core | ExecutorShared | trampoline | `TrampolineState` | Mutex | trampoline |
| actor-core | SharedMessageQueue | inner | `VecDeque<Envelope>` | Mutex | balancing queue |
| actor-core | MessageInvokerShared | inner | `Box<dyn MessageInvoker>` | RwLock | invoker pipeline |
| actor-core | MiddlewareShared | inner | `Box<dyn MessageInvokerMiddleware>` | RwLock | middleware |
| actor-core | ActorShared | inner | `Box<dyn Actor + Send + Sync>` | Mutex | actor instance |
| actor-core | DeadLetterShared | inner | `DeadLetter` | RwLock | dead letter |
| stream-core | StreamShared | inner | `Stream` | Mutex | stream instance |
| cluster-core | IdentityLookupShared | inner | `Box<dyn IdentityLookup>` | Mutex | identity |
| cluster-core | ClusterPubSubShared | inner | `Box<dyn ClusterPubSub>` | Mutex | pubsub |
| cluster-core | DeliveryEndpointShared | inner | `Box<dyn DeliveryEndpoint>` | Mutex | delivery |
| cluster-core | GrainMetricsShared | inner | `GrainMetrics` | Mutex | metrics |
| cluster-core | MembershipCoordinatorShared | inner | `MembershipCoordinator` | Mutex | membership |
| cluster-core | GossiperShared | inner | `Box<dyn Gossiper>` | Mutex | gossip |
| cluster-core | PlacementCoordinatorShared | inner | `PlacementCoordinatorCore` | Mutex | placement |
| utils-core | WaitNodeShared | inner | `WaitNode<E>` | Mutex | wait coordination |

**合計 17 個**。

#### B.6 分類サマリ

```
per-actor hot path:           10 個 (actor-core 6, stream-core 4)
per-actor non-hot path:        1 個
per-system hot path:           5 個 (actor-core 2, stream-core 3)
per-system non-hot path:      17 個
global / system-singleton:    17 個
────────────────────────────────────────
合計:                         50 個
```

#### B.7 特記事項

1. **重要: 当初想定の「153 ファイル」は過大評価**
   - `RuntimeMutex` / `RuntimeRwLock` を使う **ファイル** は ~115 だが、**field 型としての保持箇所は 50 個**
   - 残りは type alias の parameter 経由やドキュメント内言及、test で新規作成、など
2. **stream-core も hot path を持つ**
   - SinkQueue, SourceQueue, Hub 系が全て per-actor hot path
   - actor-core の Mailbox と同じ懸念 (spin 多段) が stream 側にもある
3. **RwLock read 優位の場所が多い**
   - EventStream, Scheduler, SystemState, SerializationRegistry は全て read >> write
   - spin::RwLock は read 同時取得を許すが、contention 下では writer が starve する可能性
4. **Cluster 系はほぼ lifecycle のみ**
   - cluster-core の 13 箇所全てが init/coordinate フェーズ
   - hot path は ClusterExtension 内からでも actor-core の Mailbox 経由
5. **SpinSyncMutex を直接参照している箇所** (stream-core 6 個, utils-core 内部 1 個)
   - これらは `RuntimeMutex` 経由ではなく **直接 `SpinSyncMutex` を名指し**
   - `lock-driver-port-adapter` change の対象に含める場合、`RuntimeMutex<T, SpinSyncMutex<T>>` への書き換えが必要
6. **三重ロック問題 (調査 A) の延長が stream-core にも存在する可能性**
   - SinkQueue / SourceQueue の内部実装が `SyncQueueShared` ベースなら、同じ構造
   - 本ドキュメントでは詳細調査していないが、別 change で検証価値あり

#### B.8 Port/Adapter 対象スコープ再考

lock-driver-port-adapter change の **必須対象** (factory genericization が意味ある箇所):

| カテゴリ | 数 | 理由 | 戦略 |
|---|---|---|---|
| per-actor hot path | 10 | deadlock 検知の主目的 | factory genericization |
| per-system hot path | 5 | test 時に差し替え要求あり | factory genericization |
| per-actor / per-system non-hot | 18 | test では差し替え不要 | per-crate alias (調査 A の案 a2) |
| global | 17 | init 時のみ、contention 稀 | per-crate alias |

**hot path 対象は 15 個** (当初想定 30 個から半減)。Phase 3 (actor-core/kernel ジェネリック化) のスコープをこれに合わせれば、本 change の工数は相当縮む。

### D. driver 候補と no_std lock-free queue の可否

Web 検索と docs.rs を併用して fraktor-rs の MessageQueue バリアントを lock-free / low-contention MPSC queue に置き換える候補を調査した。

#### D.1 no_std lock-free MPSC queue ライブラリ総合評価

| 候補 | no_std | alloc | bounded/unbounded | lock-free | MPSC 最適化 | 推奨度 | 備考 |
|---|---|---|---|---|---|---|---|
| `SpinSyncMutex<VecDeque>` (現状) | ✓ | ✓ | unbounded | ✗ | ✗ | △ | 2〜3 段ロック |
| `heapless::spsc::Queue` | ✓ | 不要 | bounded | ✓ | SPSC のみ | ✗ | SPSC 限定 |
| **`heapless::mpmc::Queue`** | ✓ | 不要 | bounded | △ | ✗ | **✗** | **deprecated** + 容量 ≤128 |
| `bbqueue` | ✓ | 不要 | bounded | ✓ | SPSC のみ | ✗ | SPSC + DMA 向け |
| `crossbeam_queue::ArrayQueue` | ✓ | ✓ | bounded | ✓ | ✗ (MPMC) | ○ | bounded mailbox に好適 |
| `crossbeam_queue::SegQueue` | ✓ | ✓ | unbounded | △ | ✗ (MPMC) | ○ | **内部 spinlock あり** (Issue #675) |
| `concurrent-queue` | ✓ | ✓ (global alloc) | 両対応 | ✓ | ✗ (MPMC) | ○ | portable-atomic feature あり |
| **`thingbuf::mpsc`** | ✓ | ✓ / static | bounded | ✓ | **✓ (MPSC 専用)** | **◎** | bounded のみ、MSRV 1.57 |
| `flume` | ✗ | ✓ | 両対応 | ? | ✗ | ✗ | std 必須 |
| **自作 Vyukov MPSC** | ✓ | ✓ | unbounded | ✓ | **✓ (MPSC 専用)** | **◎ (長期)** | 約 100 行、依存ゼロ |

#### D.2 主要所見

1. **`heapless::mpmc::Queue` は deprecated**
   - ドキュメントに「preemption/park で queue が使用不能になり得るため truly lock-free でない」と明記
   - 容量上限 128 (feature で 256) も Mailbox には不足
   - 既に workspace に `heapless 0.9` がある ので「ある依存を活用する」誘惑は要警戒
2. **`thingbuf::mpsc` が MPSC 専用最適化を備えた no_std 対応の唯一の完成形**
   - `Sender`/`Receiver` の MPSC 専用 API
   - `alloc` feature or `static` feature で両対応
   - `BoundedMessageQueue` の置き換えに最適
   - ただし **bounded のみ** (unbounded バリアントには使えない)
   - slot-reference API のため `Envelope: Default` 制約の可能性あり (実装前要確認)
3. **`crossbeam::SegQueue` は短期的に最小コストで移行可能**
   - `default-features = false, features = ["alloc"]` で no_std + alloc 対応
   - ただし内部 spinlock あり (完全 lock-free ではない、[Issue #675](https://github.com/crossbeam-rs/crossbeam/issues/675))
   - 実用上は `VecDeque + SpinMutex` より明確に良好
4. **自作 Vyukov MPSC が長期的ベスト**
   - protoactor-go の `mpsc.go` (67 行) と Pekko の `AbstractNodeQueue.java` (約 150 行) がそのまま参照実装
   - 外部依存ゼロ (既存 `portable-atomic` + `alloc` のみ)
   - **ただし `unsafe` を含むため `loom` / `miri` テストが必要**
   - `dequeue` は single consumer 前提 (fraktor-rs の dispatcher モデルと合致)
5. **`UnboundedDequeMessageQueue` の `enqueue_first` (stash) は lock-free MPSC では実装不可**
   - Vyukov アルゴリズムや ring buffer では front insertion ができない
   - **このバリアントだけは `SpinSyncMutex<VecDeque>` のまま残すべき** (別系統として維持)

#### D.3 MessageQueue バリアント別推奨マッピング

| バリアント | 短期推奨 | 長期推奨 | 備考 |
|---|---|---|---|
| `UnboundedMessageQueue` | `crossbeam::SegQueue` | **自作 Vyukov MPSC** | protoactor/Pekko と同方式 |
| `BoundedMessageQueue` | **`thingbuf::mpsc` bounded** | `thingbuf::mpsc` bounded | MPSC + bounded の最適解 |
| `UnboundedDequeMessageQueue` | **現状維持** (SpinMutex) | **現状維持** | stash 用 `enqueue_first` が必要 |
| `*StablePriorityMessageQueue` | 現状維持 | BinaryHeap + 専用 lock | 優先度 MPSC の lock-free 化は難しい |
| `UnboundedControlAwareMessageQueue` | 2 本の lock-free MPSC | 2 本の自作 Vyukov | Pekko `UnboundedControlAware` と同じ |

#### D.4 no_std Mutex driver 候補整理

| 候補 | 環境 | 特性 | 推奨度 |
|---|---|---|---|
| `spin::Mutex` (現状、portable_atomic feature 有効) | bare-metal | 真のスピン、再入不可、tokio 上で危険 | △ (no_std default) |
| `critical-section` | single-core bare-metal | 割り込み禁止 | ◎ (embedded) |
| `embassy_sync::blocking_mutex::{CriticalSectionRawMutex, NoopRawMutex, ThreadModeRawMutex}` | embassy executor | executor 統合 | ◎ (embedded async) |
| `portable-atomic-util::spin::Mutex` | — | **存在しない** | — (確認済み) |
| `std::sync::Mutex` | std thread | futex park | ✓ (std 非 tokio) |
| `parking_lot::Mutex` | std thread / tokio worker | 適応的 spin + park | ◎ (std 全般) |
| `tokio::sync::Mutex` | tokio worker + `.await` 跨ぎ | async-aware | ✓ (.await 専用) |

**確認事項**:
- `portable-atomic-util::spin::Mutex` は存在しない (`portable-atomic-util` が提供するのは `Arc`, `Weak`, `task::Wake` のみ)
- workspace は既に `spin 0.10 (portable_atomic)`, `heapless 0.9`, `critical-section 1.2`, `embassy-sync 0.8`, `portable-atomic 1.11` を保持
- **追加検討すべき依存**: `crossbeam-queue` または `thingbuf` (どちらを選ぶかは Phase 1 で決定)

#### D.5 結論: queue と driver は独立した 2 つの問題

`SpinSyncMutex` の根本問題 (再入不可、tokio worker 上の async デッドロック) は **Mutex の差し替えでは解決しない** — `LockDriver` port を新設しアダプタ層で driver を差し込める構造にするのが正道 (これは既に `spin_sync_mutex.rs:44-46` のコメントで予告済み)。

**2 つの問題を別 change として整理する**:

1. **Queue 実装置き換え** (Phase V 相当、別 change)
   - `UnboundedMessageQueue` → `crossbeam::SegQueue` → (将来) 自作 Vyukov MPSC
   - `BoundedMessageQueue` → `thingbuf::mpsc`
   - `UnboundedDequeMessageQueue` は据え置き
2. **Mutex driver 差し替え** (現在の `lock-driver-port-adapter` change + Phase IV)
   - `LockDriver<T>` port 導入
   - `utils-adaptor-std` で `parking_lot::Mutex` driver を提供
   - `critical-section` / `embassy_sync` は将来の `*-adaptor-embedded` で提供

この 2 つを**混ぜない**のが重要。queue 置き換えは Mailbox 層だけの変更、driver 差し替えは全 RuntimeMutex 使用箇所に及ぶ。

### B. ロック使用箇所の contention 特性

1. **153 ファイルの RuntimeMutex / RuntimeRwLock 利用を分類**
   - per-actor (contention 期待値: 極小)
   - per-system (contention 期待値: 中)
   - global (contention 期待値: 大)
2. **それぞれの hot path 判定**
   - enqueue/dequeue 経路: 極めて高頻度
   - 初期化・終了経路: 低頻度
   - 監視・metrics 経路: 中頻度

### C. 参照実装との対比

1. **Pekko の `putLock`** と fraktor-rs の `user_queue_lock` の責務マッピング
   - どちらも compound op の atomicity 保証
   - Pekko は通常 enqueue には使わない (atomic only)
   - fraktor-rs は両方に使っている → 改善余地あり
2. **protoactor-go の lock-free 戦略** が fraktor-rs に適用可能か
   - `mpsc.Queue` (Go) の実装調査
   - Rust 版 lock-free MPSC (`crossbeam::queue::SegQueue`, `flume`, etc.) の no_std 可否

### D. driver 候補の制約整理

| 環境 | 候補 | 特性 | 可否判定 |
|---|---|---|---|
| no_std bare-metal | `spin::Mutex` | busy-wait | ✓ 必須 |
| no_std embedded | `embassy::sync::Mutex`, `critical-section::Mutex` | interrupt-safe | 要検討 |
| std thread (非 async) | `std::sync::Mutex` | futex park | ✓ OK |
| std thread (非 async) | `parking_lot::Mutex` | より速い futex | ✓ 推奨 |
| tokio worker (非 .await) | `std::sync::Mutex` | 短時間なら OK | △ 条件付き |
| tokio worker (非 .await) | `parking_lot::Mutex` | 同上、より速い | △ 条件付き |
| tokio worker (.await 跨ぎ) | `tokio::sync::Mutex` | async-aware | ✓ 専用用途 |
| tokio worker (.await 跨ぎ) | sync Mutex | **禁止** | ✗ |

### E. 結論と推奨順序

調査 A (二重ロック判定) と調査 C (参照実装対比) の結果から、以下の暫定結論に至った。調査 B (contention 分類) と調査 D (no_std queue 候補) の結果を反映して最終版にする予定。

#### E.1 「二重ロック」の解消可否: 解消可能 (ただし実態は「三重」)

- fraktor-rs の `user_queue_lock` は本当は **三重ロックの最外層** で、`BoundedMessageQueue` 経路では 3 段、`UnboundedDequeMessageQueue` 経路では 2 段のネストがある
- 最外層 (`user_queue_lock`) は **案 a2** (put_lock 限定化、Pekko の `BoundedControlAwareMailbox putLock` 方式) で削減可能
- 内側 2 段 (`QueueStateHandle` と `SyncQueueShared`) の統合は別 change 候補

#### E.2 default driver の推奨: 環境別に切り替える

調査 A/C で判明した事実から、「spin 一択」ではなく環境別に default を分けるのが正しい:

| 環境 | 推奨 default | 理由 |
|---|---|---|
| no_std bare-metal / embedded | `SpinSyncMutex` | 他に現実的な選択肢がない |
| std thread (非 tokio) | `parking_lot::Mutex` or `std::sync::Mutex` | park による省 CPU、fairness |
| tokio worker | `parking_lot::Mutex` (短い crit section 前提) | spin は worker starvation のリスク |
| 長い crit section / .await 跨ぎ | そもそも lock-free 化を検討 | sync lock は不適切 |

ただし **driver 切り替えの前に lock 段数削減 (案 a2) が優先**。現状の spin のまま lock 段数を減らす方がインパクトが大きい。

#### E.3 進行順序の推奨: **β + γ 混合**

当初 4 案 (α/β/γ/δ) のうち、調査結果を踏まえた推奨順序は **β と γ の混合**:

```
Phase I: 案 γ (調査先行) — 既に完了しつつある
 └─ 本ドキュメント (調査 A/B/C/D/E)
 └─ 成果: 二重ロック削減可能性確認、参照実装との乖離把握

Phase II: 案 β (二重ロック撤廃先行)
 ├─ step II-1: Mailbox の user_queue_lock → put_lock 改名 + 限定化
 │   ├─ 通常 enqueue/dequeue/read では取らない
 │   ├─ prepend_user_messages / prepend_via_drain_and_requeue でのみ取る
 │   └─ become_closed_and_clean_up は state.close() の suspend で代替
 ├─ step II-2: contention benchmark (任意、driver swap 前でもOK)
 └─ 成果: hot path の lock 段数が Bounded 3→2, Unbounded 2→1

Phase III: 案 α 相当 (Port/Adapter)
 ├─ step III-1: 現在の openspec change (lock-driver-port-adapter) を適用
 │   ├─ LockDriver trait / RuntimeMutex<T,D> struct 化
 │   └─ actor-core を <F: LockDriverFactory> ジェネリック化
 ├─ step III-2: utils-adaptor-std に DebugSpinSyncMutex 復活
 └─ 成果: deadlock 検知 + driver 差し替え可能

Phase IV: driver 選定 (案 1)
 ├─ step IV-1: parking_lot driver 追加 (utils-adaptor-std)
 ├─ step IV-2: std thread / tokio worker 環境向け default を parking_lot に切替
 └─ 成果: std 環境での spin 浪費撤廃

Phase V (将来、別 change): Mailbox lock-free 化 (案 δ)
 ├─ step V-1: no_std 対応 Vyukov MPSC queue を自作 or crate 選定
 ├─ step V-2: MessageQueue 実装を lock-free queue ベースに差し替え
 └─ 成果: 参照実装 (Pekko/protoactor-go) と整合、hot path lock 段数 0
```

#### E.4 現在進行中の openspec change (`lock-driver-port-adapter`) の扱い

**結論: そのまま進める (Phase III に位置づける)**。ただし以下の変更を加える:

1. **proposal.md の Why セクションに「本 change 単独では lock 段数を減らさない」を明記**
2. **Non-Goals に「Mailbox の二重ロック (実は三重) の解消」「default driver の spin → parking_lot 切替」「Mailbox の lock-free 化」を追加**
3. **Open Questions に調査結果へのポインタを追加**: `docs/plan/lock-strategy-analysis.md` への参照

Phase II (二重ロック削減) は **本 change より先に別の小 change として行うべきか**、**本 change の後に続ける別 change か** を判断する必要がある。調査 B (contention 特性) で `user_queue_lock` 以外の lock 箇所で同じパターンがどの程度広がっているかを見てから決める。

#### E.5 当面のアクション (今日明日レベル)

1. **本ドキュメントを完成させる**
   - 並行実行中の調査 B (contention 分類) の結果を受けて完了
   - 並行実行中の調査 D (no_std queue 候補) の結果を受けて完了
2. **現在の openspec change (`lock-driver-port-adapter`) の proposal / design に調査結果を反映**
   - Non-Goals 追加
   - Open Questions に本ドキュメントへの参照追加
   - 「default driver の決定は別 change」を明記
3. **新規 openspec change の検討**
   - `mailbox-double-lock-reduction` (仮称): Phase II の put_lock 限定化
   - 既存 `lock-driver-port-adapter` との前後関係を決定 (先行か後続か)

#### E.6 リスクと trade-off サマリ

| リスク | 緩和策 |
|---|---|
| 現在の openspec change を進めると、後から二重ロック削減で対象 caller が変わる | change の scope を「Port/Adapter 導入」に限定し、「全 caller を factory ジェネリック化する」とは明言しない。必要最小限のみ actor-core/kernel を対象にする |
| 二重ロック削減を先行すると、deadlock 検知インフラ整備が遅れる | Phase II は小 PR で 1 週間以内に完了させる。DebugSpinSync は Phase III で導入 |
| lock-free 化 (Phase V) で no_std 対応が間に合わない | Phase V は別 change で後日、no-std 対応 Vyukov MPSC の自作 or 外部 crate (調査 D の結果次第) |
| parking_lot driver 導入 (Phase IV) で std 依存が増える | parking_lot は `utils-adaptor-std` 側に置く (utils-core は no_std 純度維持) |

---

### E.7 調査 B/D 完了後の更新

調査 B (contention 分類) と調査 D (no_std queue 候補) が完了し、当初の暫定案に以下の調整を加える。

#### E.7.1 Phase 3 の対象スコープを縮小 (調査 B より)

当初「actor-core/kernel の shared 型 30 個を factory ジェネリック化」と想定していたが、調査 B で **field 保持箇所の total は 50 個** と確定。さらに以下の内訳:

- **per-actor hot path**: 10 個 (actor-core 6 + stream-core 4)
- **per-system hot path**: 5 個 (actor-core 2 + stream-core 3)
- **hot path total**: **15 個** (当初想定 30 の半分)
- **non-hot path**: 35 個 (残り)

**Phase 3 の factory ジェネリック化対象を hot path 15 個に限定する**。non-hot path 35 個は Phase 4 の per-crate alias で済ませる。これで:

- actor-core/kernel の変更ファイル数が ~30 → ~10 に削減
- stream-core にも同様のファクトリ化が必要 (Phase 3b として追加) 、~4 ファイル
- actor-core + stream-core の hot path のみ factory 化

#### E.7.2 Queue 置き換え (新 Phase V) は独立 change で扱う (調査 D より)

調査 D の結論「queue 置き換えと Mutex driver 置き換えは独立した 2 つの問題」を採用し、**本 change のスコープから明確に除外** する:

- 本 change (`lock-driver-port-adapter`) は **Mutex driver の port/adapter 化のみ**
- MessageQueue 実装の lock-free 化は **別 change `mailbox-lock-free-queue`** (仮称)
- 優先順位は **driver 化 > queue 化** (driver 化が deadlock 検知という当面の目的を満たす)

#### E.7.3 Phase II の二重ロック削減アプローチの精緻化 (調査 A + B より)

調査 B で「stream-core にも同じ問題が潜在的にある」ことが判明した。Phase II (二重ロック削減) のスコープは:

1. **actor-core/Mailbox の `user_queue_lock` 限定化** (Pekko `putLock` 方式、調査 A の案 a2)
2. **stream-core の SinkQueue/SourceQueue/Hub 系の同型二重ロック調査** (別 change 候補、本 change 非対象)

stream-core の調査は Phase II とは別に、ボーイスカウトルール的に別 change で進める。

#### E.7.4 最終版の進行順序

```
Phase I:  調査 (本ドキュメント)                              [COMPLETED]
          └─ 二重ロック判定、参照実装対比、contention 分類、queue 候補調査

Phase II: (新規) 二重ロック削減の小 change                  [NEXT]
          ├─ actor-core/Mailbox の user_queue_lock → put_lock 限定化
          ├─ 通常 enqueue/dequeue/read は lock 取らない
          ├─ prepend/drain/close 時のみ取る
          └─ 成果: Bounded hot path 3→2 段, Unbounded 2→1 段
              影響範囲: modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs のみ

Phase III: (既存 openspec) lock-driver-port-adapter         [UPDATE NEEDED]
          ├─ proposal/design を 以下のように更新:
          │  1. Phase 3 対象スコープを hot path 15 個に限定
          │  2. stream-core にも per-crate alias + factory 化を追加 (Phase 3b)
          │  3. Non-Goals: Queue 置き換え、default driver の spin→parking_lot 変更
          │  4. Open Questions に本ドキュメントへの参照を追加
          ├─ Phase 1: LockDriver port 導入
          ├─ Phase 2: DebugSpinSyncMutex 復活
          ├─ Phase 3: actor-core/kernel hot path 15 個 + stream-core hot path 9 個 factory 化
          ├─ Phase 4: non-hot path 35 個 per-crate alias
          └─ Phase 5: test instrumentation example (deadlock 検知)

Phase IV: driver 選定 (新規 change)                         [FUTURE]
          ├─ utils-adaptor-std に parking_lot driver 追加
          ├─ std 環境向け default を parking_lot に変更
          └─ tokio 環境向け推奨を文書化

Phase V:  Queue 置き換え (新規 change)                       [FUTURE]
          ├─ UnboundedMessageQueue → crossbeam::SegQueue (短期)
          ├─ BoundedMessageQueue → thingbuf::mpsc (短期)
          ├─ 自作 Vyukov MPSC 検討 (長期、loom + miri テスト必須)
          └─ UnboundedDequeMessageQueue は現状維持 (stash の enqueue_first のため)

Phase VI: stream-core の二重ロック調査 (新規 change)         [FUTURE]
          └─ SinkQueue/SourceQueue/Hub に調査 A 同型の問題があるか検証
```

#### E.7.5 現在 pending の openspec change の具体的な更新内容

現在 `openspec/changes/lock-driver-port-adapter/` として置いてある 4 ファイルに対する具体的な改訂:

**proposal.md**:
- Why セクション末尾に「本 change は調査 A/B/C/D (`docs/plan/lock-strategy-analysis.md`) の Phase III に該当する」を追加
- Non-Goals に以下を明示:
  - Mailbox の queue 実装 lock-free 化 (別 change `mailbox-lock-free-queue` で扱う)
  - default driver の spin → parking_lot 切替 (別 change `lock-driver-parking-lot` で扱う)
  - Mailbox の二重ロック削減 (別 change `mailbox-double-lock-reduction` で扱う)
  - stream-core の二重ロック調査 (別 change で扱う)
- What Changes の Phase 3 スコープを「actor-core/kernel hot path 10 個 + stream-core hot path 4 個 (per-actor) + 5 個 (per-system)」に絞る
- Impact セクションの「影響 caller」を調査 B の数字 (50 個中 15 個が factory 化対象) に更新

**design.md**:
- Context セクションに調査結果の要約を追加
  - 実態は三重ロック (Bounded) / 二重ロック (UnboundedDeque)
  - 参照実装 (Pekko/protoactor-go) は lock-free queue ベース
  - fraktor-rs は lock-based queue ベース (設計上の乖離)
- Decision 7 の caller migration 戦略を調査 B の分類に基づいて精緻化
- Open Questions に `docs/plan/lock-strategy-analysis.md` へのポインタを追加
- Risks に「Phase V (queue 置き換え) で本 change の成果物の一部が不要になる可能性」を追加

**tasks.md**:
- Phase 3 のタスクを hot path 15 個に限定 (inventory 数字を調整)
- Phase 3b (stream-core factory 化) を追加
- Phase 4 の per-crate alias 対象を 35 個に明示

**specs/utils-lock-driver-port/spec.md**:
- 変更最小限 (既に strict valid なので、wording の微調整のみ)

#### E.7.6 短期アクション優先順位

```
優先度 1 (今日〜明日):
  ├─ 本ドキュメントを commit (調査記録)
  └─ lock-driver-port-adapter の proposal/design/tasks を上記 E.7.5 に沿って更新

優先度 2 (次の作業単位):
  ├─ Phase II: mailbox-double-lock-reduction の openspec change を新設
  └─ 実装 (modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs の書き換え)

優先度 3 (Phase II 完了後):
  └─ Phase III: lock-driver-port-adapter の実装

優先度 4 (Phase III 完了後):
  ├─ Phase IV: driver 選定 change
  └─ Phase V: queue 置き換え change
  └─ Phase VI: stream-core 調査
```

### E.8 2026-04-08 再検証による訂正と計画改訂

撤回された `remove-mailbox-outer-lock` proposal と、その後の `mailbox-close-reject-enqueue` proposal のレビューを通じて、上記の一部結論には**前提誤認**が含まれていたことが判明した。特に以下の 3 点は、**本節が旧記述を supersede する**:

1. **`prepend_via_drain_and_requeue` は production reachable**
   - `Behaviors::with_stash` / `TypedProps::from_behavior_factory` は現状 `MailboxRequirement::for_stash()` を伝播しない
   - したがって typed/classic の unstash は default mailbox でも動き、non-deque queue 上の fallback が production で実行される
   - 「dead code なので削除可能」という前提は誤り
2. **`become_closed_and_clean_up` は `state.close()` だけでは barrier にならない**
   - `MailboxScheduleState::close()` は `FLAG_CLOSED` を立てるだけで、suspend カウンタは変更しない
   - `enqueue_envelope` / `prepend_user_messages` が `is_closed()` を見ないまま進むと、close 後でも phantom enqueue が起こり得る
   - 旧記述の「close 後は `is_suspended()` で弾かれる」は誤り
3. **案 a2 (`put_lock` 限定化) は前提条件なしには unsafe**
   - close correctness を先に直さずに通常 enqueue/dequeue から outer lock を外すと、cleanup と in-flight producer の race を閉じられない
   - したがって、旧 E.3 / E.7.3 / E.7.4 の「Phase II = `put_lock` 限定化」はそのままでは採用できない

#### 改訂後の作業順序

上の再検証を踏まえると、全体計画は次の順序に改めるのが妥当:

```text
Phase I:  調査 (本ドキュメント)                                  [COMPLETED]
          └─ 三重ロック / 参照実装乖離 / contention 分類 / queue 候補

Phase II: mailbox-close-reject-enqueue                          [NEXT]
          ├─ B 案 (lock-based 再 check) を採用
          ├─ become_closed_and_clean_up を user_queue_lock で直列化
          ├─ enqueue_envelope / prepend_user_messages が lock 内で is_closed を再 check
          └─ 目的: mailbox-owned user queue mutation の close correctness を回復

Phase III: stash-requires-deque-mailbox                         [NEXT AFTER II]
           ├─ stash 利用 actor が MailboxRequirement::for_stash() を確実に伝播
           ├─ typed/classic unstash が deque-capable mailbox を獲得
           └─ 目的: prepend_via_drain_and_requeue を production unreachable に近づける

Phase III.5: mailbox-prepend-requires-deque                     [OPTIONAL BRIDGE BEFORE IV]
             ├─ `Mailbox::prepend_user_messages(...)` を deque-only 契約に硬化
             ├─ `prepend_via_drain_and_requeue` を削除
             └─ 目的: Phase IV を outer lock reduction だけに集中させる

Phase IV:  outer lock reduction の再提案                        [RE-DESIGN REQUIRED]
           ├─ Phase II/III 完了後に案 a1 / a2 を再評価
           ├─ close correctness を壊さない形で lock 段数削減を設計
           └─ 必要なら `remove-mailbox-outer-lock` を別 proposal として再作成

Phase V:   lock-driver-port-adapter                             [PARALLEL / AFTER IV]
           ├─ hot path 15 箇所を中心に factory genericization
           └─ deadlock 検知 / driver 差し替えの土台を整備

Phase VI:  driver 選定                                          [FUTURE]
           └─ parking_lot など std 環境向け default の検討

Phase VII: queue 置き換え                                       [FUTURE]
           ├─ lock-free / low-contention queue への移行
           └─ MessageQueue 実装の抜本見直し

Phase VIII: BalancingDispatcher close semantics                 [FUTURE]
            └─ shared queue は mailbox-level ではなく dispatcher-level に扱う

Phase IX:  stream-core の同型問題調査                           [FUTURE]
            └─ SinkQueue / SourceQueue / Hub 系の二重ロック検証
```

#### 改訂後の要点

1. **close correctness が outer lock 削減より先**
   - 先に `put_lock` 限定化へ進むのではなく、`Mailbox` の close と user queue mutation の race を塞ぐ必要がある
2. **stash requirement の伝播が次の前提条件**
   - `prepend_via_drain_and_requeue` を dead code 扱いするのは、`for_stash()` の伝播が実装された後でなければならない
3. **BalancingDispatcher は別トラック**
   - shared queue 経路は `Mailbox::enqueue_envelope` を通らないため、mailbox-level の close 修正と切り分ける
4. **`lock-driver-port-adapter` は継続可能**
   - ただし「Phase II = outer lock 削減」の前提は失効した。Port/Adapter change 自体は別 concern として維持する

### 最終結論

本ドキュメントの現時点の結論を 3 行でまとめると:

1. **fraktor-rs の Mailbox は参照実装から大きく乖離している** (lock-based vs lock-free, 2〜3 段ロック vs atomic)
2. **outer lock 削減の前に close correctness と stash requirement 伝播を直す必要がある** — 旧来の `put_lock` 限定化先行案は失効
3. **`lock-driver-port-adapter` は継続するが、Mailbox 周辺は `close correctness → stash requires deque → outer lock reduction` の順で進めるのが妥当**

次のアクション候補:

- **A**: `mailbox-close-reject-enqueue` を B 案前提で確定し、実装に進む
- **B**: 続けて `stash-requires-deque-mailbox` の proposal/design を作る
- **C**: Phase IV の outer lock reduction を、Phase II/III 完了後に再提案する前提で保留する

## 現時点で保留中の成果物

- `openspec/changes/lock-driver-port-adapter/proposal.md` (デフォルト型引数なし版、2026-04-08 改訂済み)
- `openspec/changes/lock-driver-port-adapter/design.md` (Bridge pattern + factory pattern + hexagonal 純度、2026-04-08 改訂済み)
- `openspec/changes/lock-driver-port-adapter/tasks.md` (Phase 1-7)
- `openspec/changes/lock-driver-port-adapter/specs/utils-lock-driver-port/spec.md` (requirements + scenarios)
- `openspec validate lock-driver-port-adapter --strict` → valid

これらは本ドキュメントの調査結果を受けて、以下のいずれかの形で扱う:

1. **そのまま進める** (案 α 採用時)
2. **二重ロック解消後にスコープ縮小して進める** (案 β 採用時)
3. **調査結果で微調整して進める** (案 γ 採用時)
4. **大幅リスコープして lock-free 移行 change として再構成** (案 δ 採用時)

## 次のアクション候補

調査を進めるかどうかの判断を user から受け取る:

- **A**: 本ドキュメントを基に案 γ で調査開始。`docs/plan/lock-strategy-analysis.md` (本ファイル) の「結論と推奨順序」セクションを順次埋めていく
- **B**: 調査を省略し、案 α で openspec change をそのまま commit/push
- **C**: 案 β で openspec change を一旦保留、二重ロック調査に切り替え
- **D**: 別案を検討

## 関連ドキュメント

- `openspec/changes/lock-driver-port-adapter/` (本件の元提案)
- `.agents/rules/rust/immutability-policy.md` (AShared パターン、ロック方針)
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Mailbox.scala` (参照実装)
- `references/protoactor-go/actor/mailbox.go` (参照実装)
- `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` (現状実装)
- `modules/utils-core/src/core/sync/spin_sync_mutex.rs` (現状の built-in driver)

## 変更履歴

- **2026-04-08**: 初版作成。3 つの課題、依存関係、4 案の比較、案 γ 推奨、調査項目の整理まで。
- **2026-04-08**: `remove-mailbox-outer-lock` / `mailbox-close-reject-enqueue` のレビュー結果を反映。`prepend_via_drain_and_requeue` の production reachability、close semantics の race、案 a2 先行の危険性を追記し、Phase 順序を改訂。
