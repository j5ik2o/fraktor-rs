# ロックフリー化の設計指針

`SharedLock` / `SharedRwLock` を剥がしてロックフリー化する際の判断基準・設計パターン・規約整合性をまとめる。

## このガイドの位置づけ

- 対象読者: ホットパスからロックを除去する設計を検討する人間・AIエージェント
- 適用範囲: アクターランタイム（mailbox / dispatcher / registry など）の同期設計
- 前提: `.agents/rules/rust/immutability-policy.md`（内部可変性ポリシー）と `.agents/rules/rust/cqs-principle.md`（CQS原則）を読了済み
- 関連ガイド: `docs/guides/shared_vs_handle.md`（Shared/Handle 命名・選択基準）

## 大原則

> **ロックフリー化とは「ロックフリーアルゴリズムを書く」ことではなく、「そもそも共有しないで済む構造に作り変える」ことである。**

```
ロックを使う      →  「同じ場所を複数スレッドが同時に触る」前提
ロックを避ける    →  「同じ場所には常に一人しか触れていない」状態を作る
```

`SharedLock` を導入する前に、**そもそも共有が必要か** を疑う。アクターランタイムは構造的に「共有しない設計」と相性が良い領域である（メッセージパッシングが基本）。

## 判断フロー

```
1. このパス（メソッド/データ）はホットか？
   ├─ No  → SharedLock / SharedRwLock のままでよい（コールドパスにロックは無害）
   └─ Yes → 次へ

2. 共有が本当に必要か？
   ├─ No  → 所有権を一人に集約する（Single-writer）
   └─ Yes → 次へ

3. 状態を読み取るだけか？
   ├─ Yes → atomic snapshot で済む（AtomicPtr / arc-swap 相当）
   └─ No  → 次へ

4. 状態変更は単純な値か、構造をまたぐか？
   ├─ 単純値    → AtomicU* + CAS で状態機械化
   ├─ 構造（map/list）→ Sharding で競合確率を 1/N に
   └─ どうしても直列化が必要 → SharedLock を残し、クリティカルセクションを縮める
```

**ホットパスの定義**: `try_send` / `dispatch` / per-message のループ内で実行される処理。秒間 10^4〜10^7 回のオーダー。

## 4つの基本パターン

| パターン | 概要 | 典型ツール |
|---|---|---|
| **① Single-writer** | 書き手を物理的に1つに限定。読み手は atomic snapshot 経由 | `AtomicPtr<T>`, RCU 風 swap |
| **② Ownership transfer** | 状態をチャネル経由で「渡す」。共有しない | lock-free MPSC |
| **③ Sharding** | キーで N 分割し、衝突確率を 1/N に | sharded `SharedRwLock` |
| **④ Atomic state machine** | 状態遷移を CAS で表現し、所有権を CAS の勝者に与える | `AtomicU8` + state encoding |

アクターランタイムでは **②と④の組み合わせ** が基本形（Pekko / protoactor-go 共通）。

### ① Single-writer

書き手を一人に限定し、読み手は immutable snapshot を atomic に取得する。

```rust
// 書き手は executor タスク1つだけ。読み手は wait-free。
struct AtomicSnapshot<T> {
    ptr: AtomicPtr<T>,
}
```

**適用例**: registry の読み取り頻度が極端に高く、書き込みが稀な場合。

### ② Ownership transfer（メッセージパッシング）

共有メモリではなく、所有権をチャネルで移送する。**アクターモデルの本質**。

```rust
// ActorCell は一度に一つの executor task が排他所有
// 「共有」ではなく「順番に渡す」構造
mailbox.queue.push(msg);  // 所有権が mailbox に移る
mailbox.run(|cell| ...);  // 排他所有権が cell の処理者に移る
```

### ③ Sharding

キーで N 分割し、各 shard に独立したロックを持たせる。

```rust
struct ShardedRegistry<K, V> {
    shards: [SharedRwLock<HashMap<K, V>>; 64],
}
```

**ロック自体は使うが、競合確率が 1/N に下がる**。`dashmap` の本質はこれ。新規依存なしで既存 `SharedRwLock` を組み合わせるだけで実装できる。

### ④ Atomic state machine

状態遷移を CAS で表現し、**勝者だけが排他所有権を得る**。

```rust
// IDLE → SCHEDULED → RUNNING → IDLE
// 各遷移は CAS。CAS の勝者だけが次の状態の権限を持つ
```

これがホットパスのロックフリー化の中核。次節で詳述する。

## CQS と内部可変性の両立（最重要）

ロックフリー化は CQS 原則と **2つの次元** で衝突する：

```
次元 A: TOCTOU レース回避のために CAS が必要
        → check と act を分離できない（Q と C の分離不可能性）

次元 B: AtomicU* に対する操作はシグネチャが &self
        → CQS が要求する「Command は &mut self」を満たせない（内部可変性問題）
```

両方とも **既存規約の枠内で正当化できる**。順に整理する。

### 次元 A: CQS の Q/C 分離不可能性

CAS は本質的に「読み取り + 書き込み」を不可分に行うため、Q と C を分離した瞬間に TOCTOU レースが発生する。

```rust
// ❌ CQS純粋だが並行下で壊れる
fn state(&self) -> u8 { self.0.load(...) }    // Query
fn set(&mut self, s: u8) { self.0.store(s, ...) }  // Command

if state() == IDLE {        // ← この瞬間
    set(SCHEDULED);          // ← この瞬間に他スレッドが先に書いてるかも (TOCTOU)
}
```

これは `Vec::pop` / `Iterator::next` と同じ系統の **「分離不可能な CQS 例外」** で、`.agents/rules/rust/cqs-principle.md` の許容例外節に該当する。

### 次元 B: 内部可変性は Shared ラッパーパターンの枠内に収める

`AtomicU*` の `compare_exchange` は `&self` で呼び出せる。これは **内部可変性** であり、`.agents/rules/rust/immutability-policy.md` は内部可変性を **「Shared ラッパーパターンが唯一の許容ケース」** と定めている。

つまり `AtomicU*` バックの型を雑に書くと、規約違反になる：

```rust
// ❌ 命名・構造が Shared ラッパーパターンに準拠していない
//   → 規約上の根拠なしに内部可変性を使っている
pub struct MailboxState(AtomicU8);

impl MailboxState {
    pub fn try_schedule(&self, on_won: impl FnOnce()) { /* &self で mutate */ }
}
```

**正しい解釈**: `AtomicU*` バックの型は **Shared ラッパーパターンの一実装** である。

> `SharedLock<T>` がロックで内部可変性を提供するのに対し、
> `AtomicU*` バックの `*Shared` 型は CAS で内部可変性を提供する。
> **両者は規約上同じ地位であり、命名・構造の規約も同じく適用される**。

| 項目 | `SharedLock<T>` バック | `AtomicU*` バック |
|---|---|---|
| 命名 | `*Shared` | `*Shared` |
| 内側の純粋ロジック | `pub struct Xyz`（`&mut self`、CQS 純粋） | `pub struct XyzInner`（`&mut self`、CQS 純粋） |
| 外側の共有ラッパー | `with_read` / `with_write` クロージャ API | `try_*` クロージャ API |
| 内部可変性の根拠 | Shared ラッパーパターン | **同左**（手段が CAS に変わるだけ） |

### 正しい2層構造

`MailboxState` を例に：

```rust
// === 1. 純粋な状態機械ロジック ===
// &mut self を取り、CQS 純粋。テスト時はこちらを直接使える。
pub struct MailboxStateInner {
    value: u8,
}

impl MailboxStateInner {
    pub fn try_schedule(&mut self) -> bool {
        if self.value == IDLE {
            self.value = SCHEDULED;
            true
        } else {
            false
        }
    }
    pub fn try_acquire(&mut self) -> bool { /* SCHEDULED→RUNNING */ }
    pub fn release(&mut self, more_messages: bool) { /* RUNNING→IDLE/SCHEDULED */ }
}

// === 2. Shared ラッパー（atomic-backed）===
// SharedLock<MailboxStateInner> の最適化版。
// &self での mutation は Shared ラッパーパターンの規約に準拠している。
pub struct MailboxStateShared {
    inner: AtomicU8,
}

impl MailboxStateShared {
    /// `SharedLock::with_write` の atomic 版。
    /// 違いは「ロックを取る → クロージャ実行 → 解放」が
    ///       「CAS で勝つ → on_won 実行」に変わっただけ。
    pub fn try_schedule(&self, on_won: impl FnOnce()) {
        if self.inner.compare_exchange(IDLE, SCHEDULED, AcqRel, Acquire).is_ok() {
            on_won();
        }
    }

    pub fn try_run<R>(
        &self,
        on_acquired: impl FnOnce() -> RunResult<R>,
    ) -> Option<R> {
        if self.inner.compare_exchange(SCHEDULED, RUNNING, AcqRel, Acquire).is_err() {
            return None;
        }
        let RunResult { value, next } = on_acquired();
        match next {
            NextState::Idle       => self.inner.store(IDLE,      Release),
            NextState::Reschedule => self.inner.store(SCHEDULED, Release),
        }
        Some(value)
    }
}
```

**この構造の利点**:
- `MailboxStateInner` は `&mut self` で CQS を完全に満たし、純粋関数として単体テスト可能
- `MailboxStateShared` は **Shared ラッパー規約** に準拠（命名 / API 形 / 内部可変性の根拠）
- 「内部可変性は Shared ラッパーパターンが唯一の許容ケース」という規約と整合
- 実装手段（ロック vs CAS）の選択は性能要件で決められ、API の形は同じ

### bool 返却 vs クロージャ API の選択

`MailboxStateInner::try_schedule(&mut self) -> bool` は **CQS違反**（Command + 値返却）。これは `Vec::pop` 同型の許容例外として認められる。

ただし **共有経由で呼ばれる API（`MailboxStateShared`）はクロージャ版に統一すべき**：

```rust
// ❌ Shared ラッパー側に bool を漏らすと「ask-then-act」を誘発
impl MailboxStateShared {
    pub fn try_schedule(&self) -> bool { ... }  // 呼び側が if 分岐する設計に逆戻り
}

// ✅ クロージャ版で「Tell, Don't Ask」を強制
impl MailboxStateShared {
    pub fn try_schedule(&self, on_won: impl FnOnce()) { ... }
}
```

### 設計原則として一般化

> **CAS を含む並行プリミティブを設計する際は：**
>
> 1. **2層構造** にする：純粋ロジック（`*Inner`、`&mut self`、CQS 純粋）+ Shared ラッパー（`*Shared`、`&self`、CAS 内部可変性）
> 2. **内部ロジック** は `&mut self` で CQS を満たし、単体テスト可能にする
> 3. **Shared ラッパー** は Shared ラッパーパターン規約に準拠（命名 / API 形）
> 4. **API 表面** はクロージャベースで「Tell, Don't Ask」を強制する
> 5. **`Option<R>` の戻り値** は `Vec::pop` 同型の許容例外
> 6. **bool 戻り値の `&self` 状態変更メソッドは設計途中の兆候** — クロージャ API へ置換

### 既存規約との整合

| 既存規約 | 本パターンとの整合 |
|---|---|
| **内部可変性は Shared ラッパーパターンが唯一の許容ケース**（`immutability-policy.md`） | `*Shared` + `*Inner` の2層構造で準拠。実装手段がロックから CAS に変わるだけ |
| **`*Shared` 命名**（`naming-conventions.md`） | atomic-backed な共有型は必ず `*Shared` を付ける |
| **`&mut self` 原則**（`cqs-principle.md`） | `*Inner` 側で完全に満たす。`*Shared` は Shared ラッパー規約の枠内で `&self` を許容 |
| `SharedAccess::with_read` / `with_write` クロージャ API | `try_schedule(\|\| ...)` / `try_run(\|\| ...)` は同じ思想の atomic 版 |
| ガード/ロックを外部に返さない | 排他権がクロージャ内に閉じる。外に漏れない |
| CQS違反は人間許可で例外許容（`Vec::pop` 相当） | `*Inner::try_*(&mut self) -> bool`、`*Shared::try_run -> Option<R>` ともに同型 |
| Tell, Don't Ask | bool 返却して呼び側で分岐する代わりに、勝った場合の動作をクロージャで渡す |

## 適用するコンポーネントの優先順位

ホットさが高い順。ロックフリー化の ROI が高いものから着手する。

| コンポーネント | 推奨パターン | 優先度 |
|---|---|---|
| Mailbox 状態機械 | ④ Atomic state machine（`*Inner` + `*Shared` の2層構造、`AtomicU8` バック） | **最高** |
| Mailbox メッセージキュー | ② Ownership transfer（lock-free MPSC） | **最高** |
| ActorCell の排他制御 | ④ の CAS 勝者にのみ `&mut` を与える | **最高** |
| Run-queue（scheduler） | std: tokio に委譲 / no_std: 自前 bounded ring | 高 |
| Registry（PID→ActorRef） | ③ Sharding（既存 `SharedRwLock` 活用） | 中 |
| Children / Watchers | 親アクターが排他所有 → ロック不要 | （該当なし） |
| Supervision strategy | 構築後 immutable → `Arc<dyn>` で十分 | （該当なし） |
| Config / membership 変更 | `SharedLock` のまま | 低（コールド） |

「ホットパスにロックがあること」が問題であり、ロックそのものが悪ではない。コールドパスは `SharedLock` のままが正解。

## 段階的移行アプローチ

ロックフリー化はバグの温床。一度にやらない。

```
Step 1. 現状の SharedLock 設計のままベンチを取る（基準線）
        → 効果測定の前提

Step 2. Mailbox 状態機械を `*Inner` + `*Shared` の2層構造へ
        → 純粋ロジック（&mut self）+ atomic-backed Shared ラッパー
        → unsafe ゼロ・依存ゼロ・最小リスク
        → これだけで SharedLock<Mailbox> が消え、規約とも整合

Step 3. Mailbox queue を lock-free MPSC へ
        → unsafe を1ファイルに局所化、loom + miri で検証

Step 4. Registry を sharded SharedRwLock へ
        → 依存ゼロ・unsafe ゼロ・既存 SharedRwLock を組み合わせるだけ

Step 5. それ以外は SharedLock のまま据え置き（コールドパス）
```

**Step 2 と Step 4 は依存ゼロ・unsafe ゼロでいきなり実施可能**。Step 3 だけは慎重に loom/miri を整備してから。

## unsafe 管理戦略

ロックフリー化で避けられない unsafe を、プロジェクト方針（`unsafe_op_in_unsafe_fn` deny）と整合させる。

```
1. unsafe は「primitive」モジュール（mpsc_queue.rs など）に局所化する
   → 公開 API は完全に safe にする

2. 各 unsafe ブロックに SAFETY コメント必須
   → 安全性条件を明文化

3. テスト戦略を3層にする
   - unit test: 通常動作
   - loom test: メモリモデル検証（`#[cfg(loom)]`）
   - miri: CI で `cargo miri test`

4. 安全性契約を unsafe 関数の SAFETY 節に明記
   → 例: pop は単一 consumer 契約。同時に複数スレッドから呼んではならない

5. lint 緩和（#[allow] 等）は人間レビュー必須
```

### unsafe を要する箇所と要さない箇所

| 箇所 | unsafe 必要性 |
|---|---|
| `AtomicU8` の状態機械 | **不要** |
| Sharded `SharedRwLock` | **不要** |
| `AtomicPtr` の単純なポインタ swap | **不要**（store/load のみ） |
| Lock-free MPSC キュー | **必要**（Box::from_raw, 生ポインタ deref） |
| RCU 風 atomic snapshot | **必要**（参照カウント操作の race） |
| CAS 勝者にのみ `&mut` を与える | **必要**（UnsafeCell 経由） |

ホットパス最適化の8割は unsafe ゼロで実現できる。unsafe は本当に必要な箇所だけに留める。

## アンチパターン

絶対に避けるべき設計。

### 1. ホットパスに SharedLock を残す

```rust
// ❌ メッセージ送信のたびに全送信者が直列化
fn try_send(&self, msg: M) -> Result<(), E> {
    self.inner.with_write(|q| q.push(msg))
}
```

**修正**: lock-free MPSC + atomic state machine へ。

### 2. ask-then-act パターン

```rust
// ❌ TOCTOU レースの温床
if mailbox.is_idle() {
    mailbox.set_scheduled();
}
```

**修正**: クロージャベースの atomic CAS API に置き換え。

### 3. ガード/ロックを外部に返す

```rust
// ❌ プロジェクト規約違反
fn lock(&self) -> SharedRwLockGuard<'_, T> { ... }
```

**修正**: `with_read` / `with_write` クロージャ API に閉じる。

### 4. 「とりあえずロックフリーで」

```rust
// ❌ コールドパスの init/shutdown を atomic 化して unsafe を増やす
fn init(&self) -> Result<(), E> {
    // 複雑な atomic 操作 + 大量の SAFETY コメント
}
```

**修正**: コールドパスは `SharedLock` で素直に書く。

### 5. 単層構造の atomic 型（Shared ラッパー規約を素通り）

```rust
// ❌ *Shared 命名でない / 純粋ロジック層が分離されていない
//   → 内部可変性の根拠（Shared ラッパーパターン）に準拠していない
pub struct MailboxState(AtomicU8);

impl MailboxState {
    pub fn try_schedule(&self) { /* &self で mutate */ }
}
```

**修正**: 2層構造にする。
- `MailboxStateInner`（`&mut self`、純粋ロジック、CQS純粋）
- `MailboxStateShared`（`&self`、CAS、クロージャ API）

これで内部可変性が Shared ラッパーパターン規約の枠内に収まる。

### 6. unsafe を広範囲にばらまく

```rust
// ❌ 公開関数が unsafe で、呼び出し側全てに契約が伝染
pub unsafe fn dispatch(&self) { ... }
```

**修正**: unsafe は primitive 内部に閉じ込め、公開 API は safe にラップ。

## 規約・参考文献

### プロジェクト規約

- `.agents/rules/rust/immutability-policy.md` — Shared / Handle パターンと内部可変性ポリシー
- `.agents/rules/rust/cqs-principle.md` — CQS 原則と許容例外
- `.agents/rules/rust/naming-conventions.md` — `*Shared` / `*Handle` 命名
- `.agents/rules/ignored-return-values.md` — 戻り値の握りつぶし禁止（CAS 結果も対象）
- `docs/guides/shared_vs_handle.md` — `SharedLock` vs `Handle` の選択基準

### 参照実装

- Apache Pekko `Mailbox.scala` — atomic state machine の代表的実装
- protoactor-go `defaultMailbox` — lock-free queue + atomic dispatch
- Tokio `tokio::sync::mpsc` — Vyukov MPSC のプロダクション実装

### アルゴリズム参考

- Vyukov MPSC: 単一 consumer 前提の lock-free linked list queue
- LMAX Disruptor: bounded ring buffer + sequence-based coordination
- Treiber stack: lock-free LIFO（system message などに有用）

## 最終チェックリスト

ロックフリー化の PR をレビューする際の確認項目:

- [ ] ホットパスからロックが除去されているか
- [ ] コールドパスは `SharedLock` のままか（無理に剥がしていないか）
- [ ] API 表面に CQS 違反が漏れていないか（`bool` 戻り値の状態変更メソッドはないか）
- [ ] クロージャパターンで排他権がスコープ内に閉じているか（ガードを外に返していないか）
- [ ] `unsafe` が primitive モジュール内に局所化されているか
- [ ] 各 `unsafe` ブロックに SAFETY コメントがあるか
- [ ] `unsafe fn` の安全性契約が doc コメントに明記されているか
- [ ] loom / miri テストが整備されているか（lock-free queue 等）
- [ ] ベンチマークで効果が確認されているか（推測でロックフリー化していないか）
- [ ] 既存規約（CQS、Shared/Handle、戻り値握りつぶし）と整合しているか
