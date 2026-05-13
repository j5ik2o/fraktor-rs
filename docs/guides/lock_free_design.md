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

## 共有プリミティブの使い分け

`SharedLock` を剥がすかどうかは、「ロックが悪いか」ではなく **その同期がホットパスにあるか** で判断する。`ExclusiveCell` は `SharedLock` の汎用置換ではない。CAS で排他 claim を取り、勝者だけに `&T` / `&mut T` を渡すための primitive である。

| primitive | 採用するケース | 採用しないケース |
|---|---|---|
| `SharedLock<T>` | 複雑な可変状態、書き込み主体、コールドパス、初期化/終了処理 | per-message のホットパス、競合が常態化するキュー操作 |
| `SharedRwLock<T>` | 読み込み主体、書き込みが稀、registry / snapshot cache | 書き込み主体、read 中に write が詰まりやすい状態 |
| `ExclusiveCell<T>` | 共有所有された `T` に対して、CAS 勝者だけが既存の `&mut T` API を実行するホットパス | 読み込み並列性が必要、再入があり得る、長時間競合する状態 |
| `AtomicU*` + `*Shared` wrapper | 1〜8 byte の離散状態、純粋遷移関数で表せる状態機械 | 大きな構造、collection、variant に付随データがある状態 |
| `ArcShared<T>` | 構築後 immutable な値、設定、strategy | 状態変更が必要な値 |

選択順は次の通り。

```
1. 共有しなくてよいか？
   ├─ Yes → 所有権を一箇所に集約し、&mut self で書く
   └─ No  → 次へ

2. 構築後 immutable か？
   ├─ Yes → ArcShared<T>
   └─ No  → 次へ

3. 1〜8 byte の Copy 状態機械か？
   ├─ Yes → AtomicU* + pure value + *Shared wrapper
   └─ No  → 次へ

4. 既存の &mut T API を CAS 勝者だけに渡せばよいか？
   ├─ Yes → ExclusiveCell<T>
   └─ No  → 次へ

5. 読み込みが圧倒的に多いか？
   ├─ Yes → SharedRwLock<T> または RCU 風 snapshot
   └─ No  → SharedLock<T>
```

### SharedLock / SharedRwLock の標準形

`SharedLock` / `SharedRwLock` を使う場合は、driver を直接固定せず `DefaultMutex` / `DefaultRwLock` を経由する。これにより `debug-locks` や `std-locks` の選択を feature flag に委ねられる。

```rust
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

pub struct Xyz {
    // state
}

#[derive(Clone)]
pub struct XyzShared {
    inner: SharedLock<Xyz>,
}

impl XyzShared {
    pub fn new(value: Xyz) -> Self {
        Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(value) }
    }
}

impl SharedAccess<Xyz> for XyzShared {
    fn with_read<R>(&self, f: impl FnOnce(&Xyz) -> R) -> R {
        self.inner.with_read(f)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut Xyz) -> R) -> R {
        self.inner.with_write(f)
    }
}
```

読み込み主体なら `SharedRwLock::new_with_driver::<DefaultRwLock<_>>(value)` を使う。書き込み主体なら `SharedRwLock` にしない。read/write の両方が結局直列化されるなら `SharedLock` の方が意図が明確である。

### ExclusiveCell の標準形

`ExclusiveCell<T>` は `fraktor_utils_core_rs::sync` の primitive で、内部に `UnsafeCell<T>` と CAS claim を持つ。`with_read` と `with_write` はどちらも同じ排他 claim を取るため、read 同士も並列化しない。これは欠点ではなく、「この値は同時に一人しか触ってはいけない」という契約を型で閉じ込めるための設計である。

```rust
use fraktor_utils_core_rs::sync::{ArcShared, ExclusiveCell, SharedAccess};

pub struct Xyz {
    // state
}

#[derive(Clone)]
pub struct XyzShared {
    inner: ArcShared<ExclusiveCell<Xyz>>,
}

impl XyzShared {
    pub fn new(value: Xyz) -> Self {
        Self { inner: ArcShared::new(ExclusiveCell::new(value)) }
    }
}

impl SharedAccess<Xyz> for XyzShared {
    fn with_read<R>(&self, f: impl FnOnce(&Xyz) -> R) -> R {
        self.inner.with_read(f)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut Xyz) -> R) -> R {
        self.inner.with_write(f)
    }
}
```

採用条件:

- 既存のロジック本体が `&mut self` / `&mut T` を要求し、その API を変えずに共有所有したい
- 同時実行は禁止で、CAS 勝者だけがクロージャ内で処理を完結できる
- `T: Send` で十分であり、`T: Sync` を要求したくない
- read 並列性より「read/write を含めて完全直列化する」ことが正しい
- 再入しない。同じ `ExclusiveCell` の `with_read` / `with_write` をクロージャ内から呼ぶと claim 待ちで spin し続ける

避けるケース:

- 読み込みを並列化したい registry / cache
- 競合が長時間続く共有 collection
- ガードを外に返したい API
- コールドパスで、`SharedLock` の方が単純な初期化/終了処理

現在の代表例は `ActorShared` と `MessageInvokerShared` である。どちらも `ArcShared::new(ExclusiveCell::new(...))` を `new` で組み立て、`SharedAccess` 経由で `with_read` / `with_write` だけを公開する。`ActorCell` の排他制御では、CAS claim の勝者だけが actor / invoker に `&mut` で入れる。

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
| 内側の型 | `pub struct Xyz`（`&mut self` メソッドを持つ可変な状態オブジェクト） | `pub enum Xyz`（不変な値型 + 純粋遷移関数 `self -> Option<Self>`） |
| 外側が内側を含む形 | `inner: SharedLock<Xyz>`（`Xyz` を構造的に保持） | `raw: AtomicU8`（`Xyz as u8` を保持。遷移関数は `Xyz` から借りる） |
| 外側のロジック呼び出し | `inner.with_write(\|x\| x.method())` | `raw.fetch_update(\|v\| Xyz::from_u8(v)?.method().map(\|s\| s as u8))` |
| 内部可変性の根拠 | Shared ラッパーパターン | **同左**（手段が CAS に変わるだけ） |

### Shared ラッパーの本質: 「内側を含む」関係

Shared ラッパーパターンの本質は **「外側が内側を構造的に含み、内側のロジックを共有経由で呼び出せるようにする」** ことである。`SharedLock<T>` 版では：

```rust
pub struct Xyz { /* state */ }
impl Xyz { pub fn do_something(&mut self) { /* logic */ } }

pub struct XyzShared {
    inner: SharedLock<Xyz>,                // ← Xyz を「含んでいる」
}
impl XyzShared {
    pub fn do_something(&self) {
        self.inner.with_write(|x| x.do_something());  // ← 内側のロジックを呼ぶ
    }
}
```

ここで重要なのは：
- `XyzShared` は `Xyz` を **構造的に含んでいる**（フィールドとして保持）
- 遷移ロジックは `Xyz` 側の単一の真実
- `XyzShared` はそれを共有経由で実行可能にする外殻にすぎない
- **`Xyz` を消したら `XyzShared` は意味を失う**（ロジックがなくなる）

この関係を atomic-backed に持ち込むには、内側を **pure value type + pure transition functions** にして、Shared 側が `fetch_update` でその関数を CAS 経由で適用する設計にする。

### 正しい構造: pure value + atomic Shared wrapper

`MailboxState` を例に：

```rust
use core::sync::atomic::Ordering::{AcqRel, Acquire};
use portable_atomic::AtomicU8;

// === 内側: 純粋な値型 + 純粋な遷移関数 ===
// 状態セマンティクスの単一の真実。並行性ゼロでテスト可能。
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MailboxState {
    Idle      = 0,
    Scheduled = 1,
    Running   = 2,
    Closed    = 3,
}

impl MailboxState {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Idle),
            1 => Some(Self::Scheduled),
            2 => Some(Self::Running),
            3 => Some(Self::Closed),
            _ => None,
        }
    }

    /// 状態遷移は self -> Option<Self> の純粋関数。
    /// 不変な値変換なので CQS の対象外（&mut self も &self もない）。
    pub fn schedule(self) -> Option<Self> {
        match self { Self::Idle => Some(Self::Scheduled), _ => None }
    }
    pub fn acquire(self) -> Option<Self> {
        match self { Self::Scheduled => Some(Self::Running), _ => None }
    }
    pub fn release(self, more_messages: bool) -> Option<Self> {
        match self {
            Self::Running => Some(if more_messages { Self::Scheduled } else { Self::Idle }),
            _ => None,
        }
    }
}

// === 外側: Atomic Shared wrapper ===
// MailboxState を atomically 保持し、その遷移関数を CAS 経由で適用する。
// 遷移ロジックは MailboxState 側にしかない。ここでは再実装しない。
pub struct MailboxStateShared {
    raw: AtomicU8,  // 中身は MailboxState as u8
}

impl MailboxStateShared {
    pub fn new(initial: MailboxState) -> Self {
        Self { raw: AtomicU8::new(initial as u8) }
    }

    /// MailboxState::schedule を atomic に適用。勝者だけが on_won を実行。
    pub fn try_schedule(&self, on_won: impl FnOnce()) {
        let result = self.raw.fetch_update(AcqRel, Acquire, |v| {
            MailboxState::from_u8(v)?.schedule().map(|s| s as u8)
        });
        if result.is_ok() {
            on_won();
        }
    }

    /// MailboxState::acquire → ユーザーロジック → MailboxState::release を CAS で適用。
    pub fn try_run<R>(
        &self,
        on_acquired: impl FnOnce() -> RunOutcome<R>,
    ) -> Option<R> {
        // acquire 相: SCHEDULED→RUNNING を CAS
        self.raw.fetch_update(AcqRel, Acquire, |v| {
            MailboxState::from_u8(v)?.acquire().map(|s| s as u8)
        }).ok()?;

        let RunOutcome { value, more_messages } = on_acquired();

        // release 相: RUNNING→IDLE/SCHEDULED を CAS
        // RUNNING は排他確保済みなので必ず成功する
        self.raw.fetch_update(AcqRel, Acquire, |v| {
            MailboxState::from_u8(v)?.release(more_messages).map(|s| s as u8)
        }).expect("RUNNING is exclusive; release must succeed");

        Some(value)
    }
}

pub struct RunOutcome<R> {
    pub value: R,
    pub more_messages: bool,
}
```

### この構造で何が解決するか

```
┌─────────────────────────────────────────────┐
│ MailboxState (pure value type)               │
│   - 状態セマンティクスの単一の真実           │
│   - 遷移ルール（schedule/acquire/release）   │
│   - 単体テスト可能（並行性ゼロ）              │
└──────────────────┬──────────────────────────┘
                   │ 借用される
                   ↓
┌─────────────────────────────────────────────┐
│ MailboxStateShared (atomic wrapper)          │
│   - MailboxState を atomically 保持          │
│   - fetch_update で遷移を atomic に適用       │
│   - MailboxState なしでは存在意義がない      │
└─────────────────────────────────────────────┘
```

| 項目 | 結果 |
|---|---|
| **内側と外側の関係性** | 外側が内側の遷移関数を借りる。**外側は内側なしでは意味を持たない** |
| **遷移ロジックの単一の真実** | `MailboxState` のみ。重複ゼロ |
| **CQS** | `MailboxState` の遷移は pure function（`self -> Option<Self>`）で CQS の対象外 |
| **内部可変性の根拠** | `MailboxStateShared` は Shared ラッパー規約に準拠（内側の値型を atomically 包む） |
| **テスト容易性** | `MailboxState::schedule()` 等は並行性ゼロで単体テスト可能 |
| **`SharedLock<T>` との対応** | `with_write(\|x\| x.method())` ↔ `fetch_update(\|v\| logic(v))` の対応関係が明確 |

**重要**: 内側を「`&mut self` で状態を持つ struct」にしないこと。それでは外側との関係が「2つの並行実装」になり、ロジックが重複する。**内側を「不変な値型 + 純粋関数」にする** のが atomic-backed Shared の正しい形。

### 既存規約との整合

| 既存規約 | 本パターンとの整合 |
|---|---|
| **内部可変性は Shared ラッパーパターンが唯一の許容ケース**（`immutability-policy.md`） | 内側の値型を外側の `*Shared` が atomic に保持し、内側の遷移関数を `fetch_update` で借りて適用する。実装手段がロックから CAS に変わるだけ |
| **`*Shared` 命名**（`naming-conventions.md`） | atomic-backed な共有型は必ず `*Shared` を付ける |
| **`&mut self` 原則**（`cqs-principle.md`） | 内側を不変な値型 + 純粋関数（`self -> Option<Self>`）にすることで、`&mut self` も `&self` も発生せず CQS の対象外となる |
| `SharedAccess::with_read` / `with_write` クロージャ API | `try_schedule(\|\| ...)` / `try_run(\|\| ...)` は同じ思想の atomic 版 |
| ガード/ロックを外部に返さない | 排他権がクロージャ内に閉じる。外に漏れない |
| CQS違反は人間許可で例外許容（`Vec::pop` 相当） | `*Shared::try_run -> Option<R>` は `Vec::pop -> Option<T>` と同型 |
| Tell, Don't Ask | bool 返却して呼び側で分岐する代わりに、勝った場合の動作をクロージャで渡す |

## enum 化できないケースの戦略

`pure value + atomic Shared wrapper` は **小さな離散状態（≤8 byte 程度の Copy 型）** にしか適用できない。状態が大きい・コレクションを含む・部分的に可変、といったケースでは別パターンに切り替える必要がある。

### 適用境界の判定フロー

```
1. 状態は Copy 可能で 1〜8 byte に収まるか？
   ├─ Yes → enum + AtomicU* + fetch_update（前述のパターン）
   └─ No  → 次へ

2. 状態は immutable に保てるか（更新時は全置換でよいか）？
   ├─ Yes → A. RCU 風 snapshot（後述）
   └─ No  → 次へ

3. 状態の主成分はコレクション（Vec / HashMap）か？
   ├─ Yes → B. Sharding（既述）or persistent collection + snapshot
   └─ No  → 次へ

4. 状態のフィールドごとに更新頻度が大きく違うか？
   ├─ Yes → C. フィールド単位に分解して各々最適な同期手段を選ぶ
   └─ No  → 次へ

5. 単純な数値カウンタ・統計値か？
   ├─ Yes → D. AtomicU* を直接使う（ラッパー不要）
   └─ No  → SharedLock<T> のまま据え置き（ホットでなければ問題ない）
```

### A. RCU 風 snapshot（大きい immutable struct）

**適用条件**: 状態が複数フィールドを持つが、更新時に全体を置換できる。読み込みが圧倒的に多く、書き込みが稀（読み : 書き ≧ 100 : 1）。

```rust
use fraktor_utils_core_rs::sync::{ArcShared, DefaultRwLock, SharedAccess, SharedRwLock};

// 内側: immutable な値オブジェクト（DDD で言う value object）
#[derive(Debug)]
pub struct RoutingTable {
    routes: Vec<Route>,
    default: Option<Endpoint>,
}

impl RoutingTable {
    pub fn lookup(&self, key: &Key) -> Option<&Endpoint> { /* pure */ }
    /// 新しい Route を加えた **新しいテーブル** を返す（self は変更しない）
    pub fn with_added(&self, r: Route) -> Self { /* clone + push */ }
}

// 外側: ArcShared<RoutingTable> を atomic に差し替える Shared ラッパー
//   - 読み込み: ArcShared を clone するだけ（refcount + 1）。lock 区間は短い
//   - 書き込み: 新規 ArcShared を作成して swap。古い snapshot は refcount 0 で解放
pub struct RoutingTableShared {
    inner: SharedRwLock<ArcShared<RoutingTable>>,
}

impl RoutingTableShared {
    pub fn read(&self) -> ArcShared<RoutingTable> {
        self.inner.with_read(|t| t.clone())  // refcount inc のみ。lock は瞬間
    }

    /// 内側の純粋関数を借りて新規 snapshot を作成し、atomic に差し替える
    pub fn update(&self, mutate: impl FnOnce(&RoutingTable) -> RoutingTable) {
        self.inner.with_write(|t| *t = ArcShared::new(mutate(&**t)));
    }
}
```

**コスト**:
- 読み込み: `ArcShared::clone`（refcount inc 1回）+ 短い RwLock read。実用上 wait-free に近い
- 書き込み: 新規 snapshot 1個分の割り当て + RwLock write。**ここでアロケーションが発生する**が、書き込みは稀なので許容
- メモリ: 古い snapshot を参照しているリーダーがいる間、新旧両方が残る（refcount 0 で解放）

**完全 wait-free にしたいなら** `arc-swap` 相当を自前で（`AtomicPtr<T>` + hazard pointer 風）実装する選択肢もあるが、複雑度が跳ね上がるので **まず brief-lock 版で実測してから判断する**。

### B. コレクションを含む状態

**適用条件**: 状態の主成分が `Vec<T>` / `HashMap<K, V>` で、要素単位の更新が頻繁。

```rust
// 選択肢 1: Sharding（書き込みも一定量ある場合・実装が単純）
//   既出の ShardedRegistry パターンを使う
pub struct ConnectionTable {
    shards: [SharedRwLock<HashMap<ConnId, Connection>>; 64],
}

// 選択肢 2: persistent collection + snapshot（読み : 書き比率が極端な場合）
//   im::HashMap などの persistent data structure は構造共有で clone が cheap
//   A の RoutingTable パターンと組み合わせる
pub struct ConfigTableShared {
    inner: SharedRwLock<ArcShared<im::HashMap<ConfigKey, ConfigValue>>>,
}
```

**選び方**:
- 書き込み頻度が高い → Sharding（A の snapshot は書き込みが重い）
- 読み込みが極端に多く全体スナップショットが欲しい → persistent + snapshot
- 単純な lookup だけで構造共有が要らない → Sharding

### C. フィールド単位への分解

**適用条件**: 1つの struct に「ホットなカウンタ」「コールドな設定」「immutable な構成情報」が混在している。これを丸ごと `SharedLock` で守ると、ホット更新がコールド読み込みと競合する。

```rust
// ❌ 一つの SharedLock で全部を守る → 更新頻度が違うフィールド同士が競合
pub struct ConnectionStateShared {
    inner: SharedLock<ConnectionState>,  // bytes_sent の更新が config 読み込みをブロック
}

struct ConnectionState {
    bytes_sent: u64,           // ホット: 受信ごとに更新
    bytes_recv: u64,           // ホット: 送信ごとに更新
    last_error: Option<Error>, // コールド: エラー時のみ
    config: ArcShared<Config>, // immutable: 接続後変わらない
}

// ✅ フィールドごとに最適な同期手段を選ぶ
pub struct ConnectionStateShared {
    bytes_sent: AtomicU64,                       // D のパターン
    bytes_recv: AtomicU64,                       // D のパターン
    last_error: SharedRwLock<Option<Error>>,     // SharedLock（コールド）
    config: ArcShared<Config>,                    // immutable（同期不要）
}
```

**判断基準**:
- 更新頻度が桁違いに違うフィールドは絶対に分ける
- ただし「同時に変わるべき」フィールドは一緒に保持する（不変条件）
- 不変条件と性能のトレードオフは設計判断（迷ったら不変条件優先）

### D. 数値カウンタ・統計値

**適用条件**: 単純なカウンタ・累計・最大最小など、原子操作プリミティブで完結する処理。

```rust
// ラッパーすら作らず、フィールドとして直接 AtomicU64 を持つ
pub struct MailboxMetrics {
    enqueued:  AtomicU64,
    dispatched: AtomicU64,
    dropped:   AtomicU64,
}

impl MailboxMetrics {
    pub fn record_enqueue(&self)    { self.enqueued.fetch_add(1, Ordering::Relaxed); }
    pub fn record_dispatch(&self)   { self.dispatched.fetch_add(1, Ordering::Relaxed); }
    pub fn record_drop(&self)       { self.dropped.fetch_add(1, Ordering::Relaxed); }
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            enqueued:  self.enqueued.load(Ordering::Relaxed),
            dispatched: self.dispatched.load(Ordering::Relaxed),
            dropped:   self.dropped.load(Ordering::Relaxed),
        }
    }
}
```

**注意点**:
- フィールド間の整合性は **保証されない**（snapshot 中に他フィールドが変わる）。総和が一致する必要がない統計値ならこれで十分
- 整合スナップショットが必要なら A の RCU パターンへ
- `Ordering::Relaxed` は計測用途には十分。同期効果が必要なら `Acquire`/`Release`/`AcqRel` を選ぶ

### E. 大きい状態機械（variant にデータが付随）

**適用条件**: enum の各 variant が固有のデータを持ち、AtomicU8 に収まらない。

```rust
pub enum Connection {
    Connecting { started_at: Instant, attempts: u32 },
    Established { since: Instant, bytes_sent: u64 },
    Closing    { reason: CloseReason },
    Closed,
}
```

選択肢：

| アプローチ | 仕組み | 向き不向き |
|---|---|---|
| **discriminant 分離** | `AtomicU8` で variant タグだけ持ち、各 variant のデータは別フィールド | データの読み出しに整合性が必要なら不可 |
| **A. RCU snapshot** | `Connection` 全体を `ArcShared` で保持し snapshot 差し替え | 書き込みが稀ならこれが第一選択 |
| **C. 分解** | hot な数値（`bytes_sent`）は AtomicU64、状態は別管理 | 設計が分かりにくくなりがち |
| **SharedLock** | hot でなければ素直にロック | コールドパスならこれで十分 |

**判断順序**: ホットでないなら `SharedLock` → 読み込みが圧倒的に多いなら RCU → 数値だけ hot で他は安定なら分解。

### 適用パターン早見表

| 状態の特徴 | 推奨パターン | アロケーション |
|---|---|---|
| 1〜8 byte の離散状態 | **enum + AtomicU* + fetch_update** | ゼロ |
| 大きい immutable struct（全置換） | **A. RCU snapshot**（`SharedRwLock<ArcShared<T>>`） | 書き込み時のみ |
| Vec / HashMap で要素更新が頻繁 | **B. Sharding** | 通常通り |
| Vec / HashMap で全体 snapshot 必要 | **B. persistent + snapshot** | 構造共有で削減 |
| ホット/コールドが混在 | **C. フィールド分解** | 各々最適化 |
| 単純な数値カウンタ | **D. AtomicU* 直接** | ゼロ |
| variant データ付き状態機械 | **E. ホットさで判断** | パターン依存 |
| ホットでない複雑な状態 | **SharedLock のまま** | 通常通り |

### アンチパターン: enum 化の無理強い

「enum で書ければゼロアロケーション」という結論を逆手に取って、**本来 enum 化すべきでない状態を無理に enum 化しない**。

```rust
// ❌ 大きいデータを 1 byte に押し込めず、
//    別フィールドの「附属データ」と組み合わせる設計にしない
pub enum ConnState { Idle, Active, Closed }

pub struct ConnectionShared {
    state: AtomicU8,
    // ↓ state と整合する必要がある「附属データ」を別管理してしまう
    last_active: SharedLock<Option<Instant>>,
    error_log:   SharedLock<Vec<Error>>,
}
// → state と附属データの整合性が壊れる読み出しが起きる（partial snapshot）
```

**修正**: 整合性が必要な状態は A の RCU パターンで丸ごと snapshot にする、もしくは整合性を諦めて C の分解で性能を取る。**「enum + 附属データ」は最悪の中間**。


## 適用するコンポーネントの優先順位

ホットさが高い順。ロックフリー化の ROI が高いものから着手する。

| コンポーネント | 推奨パターン | 優先度 |
|---|---|---|
| Mailbox 状態機械 | ④ Atomic state machine（pure value + `*Shared` atomic wrapper） | **最高** |
| Mailbox メッセージキュー | ② Ownership transfer（lock-free MPSC） | **最高** |
| ActorCell の排他制御 | ④ + `ExclusiveCell`（CAS 勝者にのみ `&mut` を与える） | **最高** |
| Mailbox メトリクス | D. AtomicU* 直接（カウンタのみ） | 高 |
| Run-queue（scheduler） | std: tokio に委譲 / no_std: 自前 bounded ring | 高 |
| Registry（PID→ActorRef） | ③ Sharding（既存 `SharedRwLock` 活用）or A. RCU snapshot | 中 |
| RoutingTable / Cluster membership | A. RCU snapshot（`SharedRwLock<ArcShared<T>>`） | 中 |
| Children / Watchers | 親アクターが排他所有 → ロック不要 | （該当なし） |
| Supervision strategy | 構築後 immutable → `ArcShared<dyn>` で十分 | （該当なし） |
| Connection / Session 状態 | E. variant データ付き状態機械（ホットさで判断） | 中〜低 |
| Config / membership 変更 | `SharedLock` のまま | 低（コールド） |

「ホットパスにロックがあること」が問題であり、ロックそのものが悪ではない。コールドパスは `SharedLock` のままが正解。

## 段階的移行アプローチ

ロックフリー化はバグの温床。一度にやらない。

```
Step 1. 現状の SharedLock 設計のままベンチを取る（基準線）
        → 効果測定の前提

Step 2. Mailbox 状態機械を pure value enum + atomic Shared wrapper へ
        → 内側は不変な値型（self -> Option<Self> の純粋遷移関数）
        → 外側は AtomicU8 + fetch_update で内側の遷移関数を atomic に適用
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
| CAS 勝者にのみ `&mut` を与える（`ExclusiveCell`） | **必要**（UnsafeCell 経由） |

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
// ❌ *Shared 命名でない / 内側の値型と関係性がない
//   → 内部可変性の根拠（Shared ラッパーパターン）に準拠していない
pub struct MailboxState(AtomicU8);

impl MailboxState {
    pub fn try_schedule(&self) { /* &self で mutate */ }
}
```

**修正**: 内側を pure value type にして、外側がそれを atomically 包む構造にする。

```rust
// 内側: 純粋な値型 + 純粋な遷移関数
pub enum MailboxState { Idle, Scheduled, Running, Closed }
impl MailboxState {
    pub fn schedule(self) -> Option<Self> { ... }
}

// 外側: 内側を atomically 保持し、内側の遷移関数を fetch_update で借りる
pub struct MailboxStateShared { raw: AtomicU8 }
impl MailboxStateShared {
    pub fn try_schedule(&self, on_won: impl FnOnce()) {
        if self.raw.fetch_update(AcqRel, Acquire, |v| {
            MailboxState::from_u8(v)?.schedule().map(|s| s as u8)
        }).is_ok() { on_won(); }
    }
}
```

### 6. 内側と外側が無関係な「並行する2実装」

```rust
// ❌ 「2層構造」を装っているが、Inner と Shared が遷移ロジックを重複保有
//   どちらも単独で完結し、お互いに必要としない = Shared ラッパーになっていない
pub struct MailboxStateInner { value: u8 }
impl MailboxStateInner {
    pub fn try_schedule(&mut self) -> bool { /* IDLE→SCHEDULED */ }
}

pub struct MailboxStateShared { inner: AtomicU8 }
impl MailboxStateShared {
    pub fn try_schedule(&self) {
        // ↓ Inner と同じロジックを CAS 版で再実装している
        self.inner.compare_exchange(IDLE, SCHEDULED, ...);
    }
}
```

**修正**: Shared ラッパーは内側を **構造的に含み**、内側のロジックを **借りる** 関係にする。
- `*Shared` の構造体フィールドが内側の値を atomically 保持する
- 遷移ルールは内側の純粋関数のみが定義する
- `*Shared` は `fetch_update` で内側の関数を渡して適用する

### 7. unsafe を広範囲にばらまく

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
- [ ] `ExclusiveCell` を read 並列化や再入が必要な場所に使っていないか
- [ ] `SharedLock` / `SharedRwLock` は `DefaultMutex` / `DefaultRwLock` 経由で初期化しているか
- [ ] `unsafe` が primitive モジュール内に局所化されているか
- [ ] 各 `unsafe` ブロックに SAFETY コメントがあるか
- [ ] `unsafe fn` の安全性契約が doc コメントに明記されているか
- [ ] loom / miri テストが整備されているか（lock-free queue 等）
- [ ] ベンチマークで効果が確認されているか（推測でロックフリー化していないか）
- [ ] 既存規約（CQS、Shared/Handle、戻り値握りつぶし）と整合しているか
