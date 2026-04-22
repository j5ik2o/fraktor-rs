## Context

**現状:**
- `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs:256` で
  `Mailbox::run(throughput, _throughput_deadline)` 引数を受け取るが `_` プレフィックスで未使用。
- `base.rs:294` に `// Deadline support is added in a follow-up change (MB-M1, Phase A3)` と明記。
- `DispatcherConfig::throughput_deadline: Option<Duration>` は既に設定可能で、
  `DispatcherCore` → `MessageDispatcherShared::throughput_deadline()` → `mbox.run(...)` の経路で
  mailbox まで届いている。**最後の 1 マイル (実際に enforce する) が欠落。**

**Pekko 参照 (`references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Mailbox.scala:261-278`):**

```scala
@tailrec private final def processMailbox(
    left: Int = java.lang.Math.max(dispatcher.throughput, 1),
    deadlineNs: Long =
      if (dispatcher.isThroughputDeadlineTimeDefined)
        System.nanoTime + dispatcher.throughputDeadlineTime.toNanos
      else 0L): Unit =
  if (shouldProcessMessage) {
    val next = dequeue()
    if (next ne null) {
      actor.invoke(next)
      processAllSystemMessages()
      if ((left > 1) && (!dispatcher.isThroughputDeadlineTimeDefined || (System.nanoTime - deadlineNs) < 0))
        processMailbox(left - 1, deadlineNs)
    }
  }
```

**既存の monotonic clock 経路 (SP-M1 以降):**
- `ActorSystem::monotonic_now() -> Duration` は既に整備済 (restart statistics, logging, actor_cell で使用)。
- `modules/actor-core/src/core/kernel/actor/actor_cell.rs:1355,1365,1460` で呼び出し例あり。
- fraktor-rs では `Duration` (Instant からの経過時間) を monotonic 時刻の表現として使用。

## Goals / Non-Goals

**Goals:**
- `Mailbox::run()` のループで Pekko `left > 1 && (nanoTime - deadlineNs) < 0` と **行単位一致** した
  条件 enforcement を実現する (fraktor-rs 側は `left > 0` で次イテレーション判定になる点に注意、
  Pekko の `left > 1 && 次 = left - 1` と意味的等価)。
- deadline 未設定 (`None`) の場合は **完全に従来挙動を保つ** (throughput-only yield)。
- monotonic 時刻源を kernel/adaptor 境界を越えて注入可能にする (no_std 維持)。
- Pekko `Mailbox.scala:261-278` への行単位対応を rustdoc に残す。
- Pekko 契約を pinned する新テストを追加 (deadline 超過 / 未達 / None 三種)。

**Non-Goals:**
- MB-M2 (BoundedDequeBasedMailbox / BoundedControlAwareMailbox) の追加実装。
- MB-M3 (blocking push-timeout、Rust 設計外) の実装。
- `throughput_deadline` の dispatcher config 上の解釈変更 (既に `Option<Duration>` で伝わっている)。
- AC-M1 / AC-M2 (pinned 排他 / alias 解決) の改修。

## Decisions

### Decision 1: clock 注入方式 — `Mailbox` に `Arc<dyn Fn() -> Duration>` 相当の callback を持たせる

**選択肢:**

A. **引数注入**: `run(throughput, deadline, now_fn: impl Fn() -> Duration)`
B. **ActorSystem 参照**: `Mailbox` に `Weak<ActorSystemState>` を持たせて `system.monotonic_now()` を呼ぶ
C. **dispatcher 側で deadline 時刻を計算して `deadline_at: Option<Duration>` を mailbox に渡す**
D. **clock callback フィールド**: `Mailbox` に `Arc<dyn Fn() -> Duration + Send + Sync>` を保持

**採用: D (clock callback フィールド)**

**採用理由:**
- `Mailbox::run()` の各ループイテレーションで `now()` を呼ぶ必要があるため、引数経由 (A)
  は dispatcher 側呼び出しの複雑化を招く。
- B (ActorSystem 参照) は結合度が高く、mailbox が `ActorSystemState` に直接依存する設計になる。
  SP-M1 で `restart_statistics` が `monotonic_now` を引数で受け取る設計を選んだのと一貫性が取れない。
  さらに **既存 `SystemState::monotonic_now` (`system_state.rs:863`) は `AtomicU64::fetch_add(1)`
  を毎回インクリメントするカウンタベース実装** で wall-clock 経過時間を返さないため、throughput
  deadline の経過時間判定には機能しない (instrumentation 用途専用)。deadline enforcement には
  `std::time::Instant::now()` 起点の実時間 closure が別途必要であり、この点からも B は不適。
- C (dispatcher が deadline 時刻計算) は deadline の時刻評価を一か所に集約できるが、
  Pekko `Mailbox.scala:262-266` では **mailbox 側** で deadline 計算しているため parity から外れる。
  また `monotonic_now()` 自体を何度も呼ぶ (ループ内判定) 必要があるため、時刻取得責務を
  dispatcher に寄せると責務分離が崩れる。
- D (clock callback フィールド) は Mailbox::new() で一度だけ注入し、以降は各 run() で `(clock)()`
  を呼ぶだけで済む。no_std 対応 (`Arc<dyn Fn() -> Duration + Send + Sync>` は alloc で成立)、
  テストでは mock clock を注入可能、Pekko の「mailbox 自身が時刻を取得する」責務配置と整合。

**型:**
```rust
pub type MailboxClock = alloc::sync::Arc<dyn Fn() -> core::time::Duration + Send + Sync>;
```

#### 既存 factory コンストラクタの扱い (production 経路)

`base.rs:68-187` に **8 本** の public factory が存在する:

1. `Mailbox::new(policy)`
2. `Mailbox::new_with_shared_set(policy, shared_set)`
3. `Mailbox::new_from_config(config)`
4. `Mailbox::new_from_config_with_shared_set(config, shared_set)`
5. `Mailbox::new_sharing(policy, queue)`
6. `Mailbox::new_sharing_with_shared_set(policy, queue, shared_set)`
7. `Mailbox::with_actor(actor, policy, queue)`
8. `Mailbox::with_actor_and_shared_set(actor, policy, queue, shared_set)`

**採用方針: `MailboxSharedSet` に `clock: Option<MailboxClock>` field を追加。`None` は
「deadline enforcement 無効 (throughput-only fallback)」を表す sentinel。** 既存 signature は維持。

- すべての factory は最終的に `new_with_queue_and_shared_set(policy, queue, shared_set)` に集約されるため、
  `MailboxSharedSet::clock()` 経由で `Mailbox.clock` field (型: `Option<MailboxClock>`) を埋める経路 1 点に責務が集約される。
- `MailboxSharedSet::builtin()` は `clock = None` で構築される (no_std core 側の default 挙動)。
  std adaptor 側の `ActorSystem` 初期化 factory が `MailboxSharedSet::with_clock(Arc<Fn() -> Duration>)` で
  monotonic clock を **注入** する。embedded adaptor も同様の注入を行う。
- `builtin()` を直接呼ぶ factory (`new(policy)` / `new_from_config(config)` / `new_sharing(policy, queue)` /
  `with_actor(actor, policy, queue)` の 4 本) は clock=None の `MailboxSharedSet` を得るため、
  deadline enforcement が適用されない (= 従来挙動と完全一致)。本番経路は `ActorSystem` 由来の
  `*_with_shared_set` factory を通って clock=Some(...) が注入される。
- `Mailbox::run()` のループ条件は以下で Pekko 互換性を保つ:
  - `clock = None` → deadline enforcement 無効、throughput-only yield (Pekko `isThroughputDeadlineTimeDefined = false` と同値)
  - `clock = Some(_)` + `throughput_deadline = None` → throughput-only yield
  - `clock = Some(_)` + `throughput_deadline = Some(d)` → `left > 0 && now() < deadline_at` の合成条件

**トレードオフ:**
- 元案 (`Mailbox::new_with_clock` 一本化) は constructor 側のシグネチャで clock を可視化できるが、
  8 本の factory 全てに clock 引数追加 → 呼び出し箇所の破壊的書き換えが大量に発生する。
- 採用案は `MailboxSharedSet` に責務を集約することで、clock 配送の見通しを `shared_set.clock()`
  アクセスの 1 か所に集中させ、破壊的変更を `MailboxSharedSet` の field 追加のみに抑える。
- `Option<MailboxClock>` を採用することで no_std core でも panic なく `builtin()` を呼べる
  (clock=None なら deadline 判定自体をスキップ)。adaptor 層での注入漏れがあっても
  fail-safe (従来挙動に degrade)。
- **`MailboxSharedSet::new` の `const fn` 喪失**: `Arc<dyn Fn()>` を含む
  `Option<MailboxClock>` は Rust stable の const context で構築できないため、
  現行 `pub(crate) const fn new(put_lock)` は通常関数に降格する。`put_lock` 単独時に
  const fn だった利点は失われるが、`new` の呼び出し経路は kernel 内部に限定されており
  性能・可用性への影響は極小。CLAUDE.md 方針 (後方互換不要) に則り破壊的降格を許容。
- テストで mock clock を注入する場合は `MailboxSharedSet::with_clock(mock_clock)` 経由で
  `*_with_shared_set` factory を使う。または mailbox 構築後に `Mailbox::set_clock(&mut self, Option<MailboxClock>)`
  で直接差し替える (CQS: Command)。

### Decision 2: deadline 計算タイミング — `run()` 先頭で一度だけ

Pekko `Mailbox.scala:262-266` は `processMailbox` の **デフォルト引数** として `deadlineNs` を評価
するため、再帰呼び出し内で再計算されない。fraktor-rs 実装も Pekko に合わせ、`run()` の先頭で

```rust
let deadline_at: Option<Duration> = self.clock
    .as_ref()
    .zip(throughput_deadline)
    .map(|(c, d)| c() + d);
```

を **一度だけ** 計算し、`process_mailbox(invoker, throughput, deadline_at)` へ引き渡す。
`self.clock: Option<MailboxClock>` が `None` もしくは `throughput_deadline` が `None` の
どちらか片方でも成立すれば `deadline_at = None` となり、deadline 判定が完全に
スキップされる (Pekko `isThroughputDeadlineTimeDefined = false` 相当、throughput-only fallback)。

**ループ構造は Decision 4 に一本化する** (Pekko `Mailbox.scala:275` に準拠した
「invoke 後・`left -= 1` 後に `if break` する post-decrement 構造」)。Decision 2 は
ループ構造ではなく `deadline_at` 計算式のみを規定する。

**Pekko 行との等価性:** `deadline_at = Some(_)` が成立するのは `self.clock = Some(_)` のときだけ
であるため、ループ内 break 判定の内側 `self.clock.as_ref().is_some_and(...)` は実質的に
`self.clock.as_ref().unwrap()` と等価。型安全性を保つため `is_some_and` で書く。

### Decision 3: `Mailbox::run()` シグネチャの扱い

- 既存 `run(throughput: NonZeroUsize, _throughput_deadline: Option<Duration>) -> bool` の
  `_` プレフィックスを **削除** し、内部で実際に使用する。
- シグネチャ自体は変更なし (引数の型・順序は維持)。
- 呼び出し側 (`message_dispatcher_shared.rs:305`) の変更は不要。

### Decision 4: `process_mailbox` ループ条件の Pekko 行単位写像

Pekko は `left > 1 && deadline 未達 → left-1 で再帰` という記述。fraktor-rs は while ループで
`left > 0` 単独条件だったものを、以下に変更する:

```rust
fn process_mailbox(
  &self,
  invoker: &MessageInvokerShared,
  throughput: NonZeroUsize,
  deadline_at: Option<Duration>,  // <-- 新規
) {
  let mut left = throughput.get();
  while left > 0 && self.should_process_message() {
    let Some(envelope) = self.dequeue() else { break; };
    // ... invoke + process_all_system_messages ...
    left -= 1;
    // Pekko Mailbox.scala:275: `(left > 1) && (!deadlineDefined || (nanoTime - deadlineNs) < 0)`
    // fraktor-rs は post-decrement の while loop なので `left > 0` (= Pekko の left > 1 で再帰進行と等価)
    // deadline 判定のみ追加。
    //
    // Safety/invariant: `deadline_at` は `run()` 先頭で
    //   self.clock.as_ref().zip(throughput_deadline).map(|(c, d)| c() + d)
    // として計算される (Decision 2)。つまり `deadline_at = Some(_)` は型レベルで
    // `self.clock = Some(_)` と同値 (Option::zip が片方 None なら None を返すため)。
    // したがって下の `is_some_and` は論理的には `.unwrap()` と等価だが、
    // 型安全性と読みやすさのため明示的に書く (冗長な二重チェックを許容)。
    if let Some(da) = deadline_at
        && self.clock.as_ref().is_some_and(|c| c() >= da)
    {
      break;
    }
  }
}
```

clock 呼び出しが `deadline >= da` で true になる条件、すなわち **deadline ちょうど (==) で break** する。
Pekko は `nanoTime - deadlineNs < 0` すなわち「deadline 未達」条件で継続、「deadline 到達または超過」で break (当方と同じ)。

**ループ exit の 2 経路の等価性:**

上記擬似コードには 2 つの終了経路が存在する:

1. `left == 0` で `while` 条件が false になって終了 (throughput 消化)
2. `if let Some(da) = deadline_at && self.clock.as_ref().is_some_and(|c| c() >= da) { break; }` で break (deadline 到達)

`throughput = 1` かつ `deadline = Some(Duration::ZERO)` の場合、両経路が同時に成立する:
- invoke 1 回 → `left -= 1` で `left = 0` → `if` 判定が実行される → clock が `deadline_at` に到達していれば break、していなくても次ループ先頭の `while` 条件で終了

どちらの経路で抜けても **「1 メッセージ処理済」** という観測結果は同一で、Pekko `left > 1 && ...`
と等価。mock clock を固定した状態では `if` 経路は踏まれず `while` 経路で抜けるが、これは
「1 通処理後に 2 通目の処理を行わない」Pekko 契約の保証として十分。仮に `if` 経路が dead code
 に見えても、それは `throughput = 1` の境界ケース限定であり、`throughput ≥ 2` では
`if` 経路が deadline 判定として実効的に動く。

### Decision 5: テストで注入する mock clock

テスト (`modules/actor-core/src/core/kernel/dispatch/mailbox/base/tests.rs`) は std 環境前提
 (`#[cfg(test)]`) で走るため、mock clock の実装に no_std 互換は不要。それでも
`SpinSyncMutex<Duration>` を採用する理由は以下の 2 点:

1. **既存ユーティリティの再利用**: kernel 内の他テストは既に `SpinSyncMutex` を使っており、
   `std::sync::Mutex` / `parking_lot::Mutex` など複数のロック実装をテストコードに混在させると
   レビュー時の認知負荷が増える。
2. **production 同型パスの維持**: mock clock 経由で注入される `Arc<dyn Fn()>` は
   production と同じ `MailboxClock` 型で、中身が `SpinSyncMutex` であっても
   `Send + Sync` 境界は等価。mock の型だけ `std::sync::Mutex` に差し替えると
   ロックセマンティクスの違い (poisoning, reentrancy) がテスト挙動に影響する可能性がある。

mock clock API:
```rust
let mock = MockClock::new(Duration::ZERO);
let clock: MailboxClock = mock.as_mailbox_clock();
// ... in test ...
mock.advance(Duration::from_millis(5));
```

**代替案の却下理由:**
- `AtomicU64` でナノ秒保持: `Instant` からの変換に余計な計算が入り、`Duration` の直接操作ができず可読性が落ちる。
- `std::sync::Mutex<Duration>`: poisoning セマンティクスが production と異なり、panic 系テストとの相性が悪い。

### Decision 6: Pekko 行単位 rustdoc の粒度

SP-M1 と同じ粒度で、`process_mailbox` の各文に `// Pekko: <Scala 該当行>` を付ける。特に:

- deadline 計算: `// Pekko Mailbox.scala:263-266`
- ループ条件: `// Pekko Mailbox.scala:275`
- yield (break): `// Pekko: if ((left > 1) && (...).isNegative) recurse — 本実装は while で左辺偽 = break`

## Risks / Trade-offs

**リスク 1: clock callback が hot loop で呼ばれる**
- `deadline_at = Some(_)` の場合、各イテレーションで `self.clock.as_ref().is_some_and(|c| c() >= da)` が呼ばれる (= `monotonic_now()` = `Instant::now()` 相当)。
- `Instant::now()` は nanoseconds 精度の monotonic clock で、一回のコストは数十ナノ秒程度。
- throughput=100 のケースでも 100 回呼び出しで 数 µs、I/O 境界の actor では誤差範囲。
- **軽量アクター (< 1µs per message)** では相対オーバーヘッドが高いが、その場合は
  `throughput_deadline = None` を設定すれば従来と完全に同じ挙動 (時刻取得ゼロ) を維持可能。
- トレードオフ: **deadline 精度 ↔ deadline 未使用時のゼロオーバーヘッド**。`None` 早期リターンで後者を担保。

**リスク 2: `Arc<dyn Fn() -> Duration + Send + Sync>` の動的 dispatch コスト**
- ループ内で vtable ポインタ経由の call が発生。
- 静的 dispatch (ジェネリクス) にすると Mailbox 全体がジェネリクスに汚染され、公開 API の複雑化を招く。
- 動的 dispatch のコストはキャッシュ済み vtable で ≪ 1ns、許容範囲。

**リスク 3: `SyncOnce<T>` フィールド所有の制約**
- `Mailbox` は `SyncOnce<MessageInvokerShared>` / `SyncOnce<WeakShared<ActorCell>>` /
  `SyncOnce<MailboxInstrumentation>` の write-once 型を 3 フィールド持つ (`base.rs:34-60`)。
- `SyncOnce<T>` は write-once セマンティクスにより `Clone` を提供しないため、`Mailbox` 全体として
  `Clone` を派生できない。
- 従って builder pattern `with_clock_override(self, clock) -> Self` はフィールド再構築が必要で危険。
- 採用: `pub(crate) fn set_clock(&mut self, clock: Option<MailboxClock>)` (CQS 原則: Command =
  `&mut self + ()`) に統一。**可視性は `pub(crate)`**: 外部クレートからの clock 差し替えは
  禁止し、production 経路は `MailboxSharedSet::with_clock(mock)` + `*_with_shared_set` factory
  のみを使う。テスト (kernel 内部) では `set_clock` で構築後に差し替え可能。

**リスク 4: no_std 環境での clock 注入**
- no_std core は `ActorSystem::monotonic_now()` の実装を持たない (std 側で `Instant::now()` を使用)。
- embedded ユーザーは `fraktor-actor-adaptor-<embedded>` 相当で `embassy-time::Instant` 等を注入。
- 本 change は core 側で `MailboxClock` type alias を定義するだけで、実装は adaptor 側に残す。

**トレードオフ: 設計複雑度 ↔ Pekko parity**
- clock 注入なしで dispatcher 側から `run(deadline_at)` を渡す案 (Decision 1 の C) より複雑。
- しかし Pekko の「mailbox 自身が時刻を取得する」責務配置と整合し、将来的な mailbox 側時刻利用
  (metrics / tracing) の拡張にも対応しやすい。
