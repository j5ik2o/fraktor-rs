## Context

PR #1530 (utils-sync-collapse) で `SyncMutexLike` / `SyncRwLockLike` trait を「1-impl の幽霊抽象」として削除した。当時は `SpinSyncMutex` / `SpinSyncRwLock` 以外の impl が無く、generic bound caller も `SyncQueueShared` のみで、YAGNI 原則に従い trait を撤廃して `RuntimeMutex<T> = SpinSyncMutex<T>` の type alias に集約した。

その後 PR #1535 / PR #1537 で `DebugSpinSyncMutex` (再入検知) を utils-adaptor-std 層に追加したが、`actor-core/kernel` の concrete `RuntimeMutex<T>` 呼び出しを差し替える接合点が無いため **実用にならない** ことが判明した。Mailbox が:

```rust
pub struct Mailbox {
  user_queue_lock: ArcShared<RuntimeMutex<()>>,           // = SpinSyncMutex<()>
  invoker:         ArcShared<RuntimeMutex<Option<...>>>,   // = SpinSyncMutex<...>
  // ...
}
let _guard = self.user_queue_lock.lock();                  // SpinSyncMutex::lock 直リンク
```

の形で SpinSyncMutex に固定されているため、test 時に DebugSpinSyncMutex に置換できない。PR #1538 で DebugSpinSyncMutex 実装は一旦 OFF し、`utils-adaptor-std` crate skeleton のみ残した上で、本 change で port 契約を整備して再導入する方針とした。

```
現状 (PR #1538 後):
  caller (115 files)
       │
       │ RuntimeMutex<T>           ← type alias
       ▼
   SpinSyncMutex<T>                ← concrete leaf, no port
       │
       │ inherent .lock()
       ▼
   spin::Mutex<T>::lock()

差し替え不可能。test 時に DebugSpinSyncMutex に置換する接合点が無い。
```

```
本 change 後:
  caller (Phase 3 対象: actor-core/kernel)
       │
       │ Mailbox<F: LockDriverFactory>        ← F は no default
       │   field 型: RuntimeMutex<T, F::Driver<T>>
       ▼
   RuntimeMutex<T, D> { driver: D, ... }      ← port struct, D は型引数のみ
       │
       │ self.driver.lock()                    ← LockDriver trait method
       ▼
   <D as LockDriver<T>>::lock(&self.driver)
       │
       ├─ production: ActorSystem<SpinSyncFactory>      → D = SpinSyncMutex<T>
       └─ test:       ActorSystem<DebugSpinSyncFactory> → D = DebugSpinSyncMutex<T>

  caller (Phase 4 対象: cluster/persistence/stream/actor-adaptor-std)
       │
       │ KernelMutex<T>                        ← per-crate alias
       │   = RuntimeMutex<T, SpinSyncMutex<T>> ← alias は caller crate 側
       ▼
   RuntimeMutex<T, D>                          ← port struct は alias を知らない
```

## Goals / Non-Goals

**Goals:**

- `LockDriver<T>` / `RwLockDriver<T>` を port 契約 (trait) として utils-core に新設
- `RuntimeMutex<T, D>` / `RuntimeRwLock<T, D>` を struct 化し、**port 定義ファイルが adapter 実装を一切参照しない** 設計にする
- `LockDriverFactory` / `RwLockDriverFactory` で多 T フィールド (Mailbox 等) のジェネリック化を可能にする
- `utils-adaptor-std` から adapter driver (`DebugSpinSyncMutex`, etc) を差し込める形にする
- actor-core/kernel の shared 型を `<F: LockDriverFactory>` ジェネリック化し、test 時の instrumentation を実用化

**Non-Goals:**

- **`pub struct RuntimeMutex<T, D = SpinSyncMutex<T>>` の default type parameter 導入**: hexagonal 原則違反なので採用しない
- **`SpinSyncMutex` を utils-core から追い出す**: 本 change ではファイル分離で port 純度を担保し、crate 分離は将来の別 change とする
- `parking_lot::Mutex` / `std::sync::Mutex` driver の追加 (将来の別 change)
- workspace 全体の feature flag による global driver swap (per-call-site swap が可能なので不要)
- `RuntimeMutex` の rename (現在の名前を維持)
- `SyncMutexLike` / `SyncRwLockLike` trait の名前の復活 (新名 `LockDriver` / `RwLockDriver` を採用)
- `SpinSyncMutex` の inherent API 削除 (LockDriver impl から delegate)
- cluster-core / remote-core / stream-core / persistence-core kernel の **factory ジェネリック化** (本 change では per-crate alias で互換維持、factory 化は別 change)
- `SharedAccess` (`with_read` / `with_write`) trait への変更 (別 concern)

## Decisions

### 1. Bridge / Strategy パターン (struct + trait) を採用する

直接 `pub type RuntimeMutex<T> = SpinSyncMutex<T>` を `pub trait RuntimeMutex<T>` に置き換えると、**115 ファイル** が一斉に壊れる:

- `ArcShared<RuntimeMutex<T>>` ← trait は型ではないので使えない (`dyn` で書けるが GAT 対応せず)
- `RuntimeMutex::new(value)` ← associated function を呼ぶには generic context が必要
- `mutex.lock()` ← どの impl の `lock` か曖昧、generic bound 必須

このため **bridge / strategy パターン** (struct が trait を内包) を採用する:

```rust
// port 契約 (trait) — utils-core/src/core/sync/lock_driver.rs
pub trait LockDriver<T>: Sized {
  type Guard<'a>: Deref<Target = T> + DerefMut where Self: 'a, T: 'a;
  fn new(value: T) -> Self;
  fn lock(&self) -> Self::Guard<'_>;
  fn into_inner(self) -> T;
}

// port struct (caller が型として使える) — utils-core/src/core/sync/runtime_mutex.rs
// 注意: この定義ファイルは SpinSyncMutex 等の具象 driver を一切 use / 参照しない
pub struct RuntimeMutex<T, D>
where D: LockDriver<T>,
{
  driver: D,
  _pd:    PhantomData<T>,
}

impl<T, D: LockDriver<T>> RuntimeMutex<T, D> {
  pub fn new(value: T) -> Self { Self { driver: D::new(value), _pd: PhantomData } }
  pub fn lock(&self) -> D::Guard<'_> { self.driver.lock() }
  pub fn into_inner(self) -> T { self.driver.into_inner() }
}
```

caller は `D` を必ず指定する。utils-core は driver の具象型を一切知らない。

代替案:

- **A. Trait に直接 rename** (`pub trait RuntimeMutex<T>`): 115 ファイルの構文が壊れる、却下
- **B. dyn dispatch** (`Box<dyn LockDriver<T>>`): GAT は dyn-incompatible (guard 型を abstract できない)、却下
- **C. 別名で trait を新設し、type alias は据え置き** (`trait LockDriverPort` 等): 名前空間が分かりにくい、却下
- **D. default type parameter `D = SpinSyncMutex<T>`**: hexagonal 原則違反 (port が adapter を名指しする)、**却下**

### 2. port 定義ファイルと built-in driver ファイルを **物理的に分離する**

「port は adapter を知らない」という原則を crate 単位ではなく **ファイル単位** で担保する。`SpinSyncMutex<T>` は本 change では utils-core に据え置くが、port 定義ファイルとは明確に分ける:

```
modules/utils-core/src/core/sync/
├── lock_driver.rs            ← port trait     (SpinSyncMutex を参照しない)
├── rwlock_driver.rs          ← port trait     (同上)
├── lock_driver_factory.rs    ← port trait     (同上)
├── runtime_mutex.rs          ← port struct    (同上、LockDriver trait のみに依存)
├── runtime_rwlock.rs         ← port struct    (同上)
├── spin_sync_mutex.rs        ← built-in driver (LockDriver impl をここに置く)
├── spin_sync_rwlock.rs       ← built-in driver (RwLockDriver impl をここに置く)
├── spin_sync_factory.rs      ← built-in factory (LockDriverFactory impl, Driver<T> = SpinSyncMutex<T>)
└── spin_sync_rwlock_factory.rs ← built-in factory (同上)
```

これにより **`runtime_mutex.rs` をレビューすれば port struct が adapter を参照していないことが機械的に確認できる**。`spin_sync_*.rs` 側は、trait impl のみで port struct には触れない。

**`SpinSyncMutex` を別 crate に出すかどうか**は本 change では判断を保留する。物理分離で原則は担保できており、将来 `fraktor-utils-adaptor-spin-rs` に切り出す価値が判明したら別 change で対応する (YAGNI)。

代替案:

- **A. `SpinSyncMutex` を新 crate に切り出す** (`fraktor-utils-adaptor-spin-rs` など): no_std 環境用の built-in driver をどこに置くかという crate 数増加問題を招く。本 change では据え置く
- **B. `SpinSyncMutex` を `utils-adaptor-std` に移動**: no_std 制約 (utils-adaptor-std は std-only) に反するので却下

### 3. `LockDriverFactory` で多 T フィールドの HKT を表現

`Mailbox` のような shared 型は、複数の異なる T を持つ:

```rust
pub struct Mailbox {
  user_queue_lock: ArcShared<RuntimeMutex<()>>,
  invoker:         ArcShared<RuntimeMutex<Option<MessageInvokerShared>>>,
  actor:           ArcShared<RuntimeMutex<Option<WeakShared<ActorCell>>>>,
}
```

各 field の `T` は異なる (`()`, `Option<...>`, `Option<...>`)。これらを **同じ driver kind で揃えたい** (production は全部 SpinSync、test は全部 Debug) ときに、単純な `<D: LockDriver<T>>` では `T` が違うので 1 つの D で共有できない。

HKT の代わりに **`LockDriverFactory` trait** で type family を表現する:

```rust
// port trait — utils-core/src/core/sync/lock_driver_factory.rs
// 具象 driver を一切参照しない
pub trait LockDriverFactory {
  type Driver<T>: LockDriver<T>;
}

// built-in factory — utils-core/src/core/sync/spin_sync_factory.rs
pub struct SpinSyncFactory;
impl LockDriverFactory for SpinSyncFactory {
  type Driver<T> = SpinSyncMutex<T>;
}

// adapter factory — utils-adaptor-std/src/std/debug/debug_spin_sync_factory.rs
pub struct DebugSpinSyncFactory;
impl LockDriverFactory for DebugSpinSyncFactory {
  type Driver<T> = DebugSpinSyncMutex<T>;
}
```

Mailbox は **factory パラメータを 1 つ** だけ取る (**default なし**):

```rust
pub struct Mailbox<F: LockDriverFactory> {
  user_queue_lock: ArcShared<RuntimeMutex<(), F::Driver<()>>>,
  invoker:         ArcShared<RuntimeMutex<Option<MessageInvokerShared>, F::Driver<Option<MessageInvokerShared>>>>,
  // ...
}
```

production 呼び出しは `Mailbox<SpinSyncFactory>` を **caller 側で明示**する (`ActorSystem::new` のような factory boundary で固定)。test では `Mailbox<DebugSpinSyncFactory>` 経由で **全 lock を一括で debug 化** できる。

代替案:

- **A. Field ごとに driver を指定** (`Mailbox<D1, D2, D3>`): generic param 数が爆発、却下
- **B. macro generation** (`define_mailbox!(driver = Spin)`): macro で複雑度が増す、却下
- **C. 全部 inline で書く** (`Mailbox<()>`, `Mailbox<DebugSpinSync>` 等): factory パターンが結局必要、本決定と等価

### 4. `RuntimeMutex` という名前を維持する

`RuntimeMutex` という名前は本来「runtime に応じて選ばれる Mutex」を意味するが、現在は `SpinSyncMutex` の alias にすぎない。本 change で **driver 選択肢が複数になる** ことで、本来の意味に復帰する:

- production: SpinSyncMutex driver
- test: DebugSpinSyncMutex driver
- 将来: parking_lot driver, std::sync driver, etc.

caller の認知負荷を最小化するため、既存の名前を維持する。

代替案:

- **A. `MutexCell<T, D>` などに rename**: 115 caller の更新が必要、却下
- **B. 別名 `DriverMutex<T, D>` を新設し、`RuntimeMutex` を deprecate**: 名前空間が二重化、却下

### 5. `SpinSyncMutex` / `SpinSyncRwLock` の inherent API は維持する

`SpinSyncMutex<T>` の直接利用 (`SpinSyncMutex::new(...)`, `mutex.lock()`) は維持する。`LockDriver` impl は inherent method への薄い委譲。

```rust
// modules/utils-core/src/core/sync/spin_sync_mutex.rs
impl<T> LockDriver<T> for SpinSyncMutex<T> {
  type Guard<'a> = spin::MutexGuard<'a, T> where T: 'a;
  fn new(value: T) -> Self { SpinSyncMutex::new(value) }
  fn lock(&self) -> Self::Guard<'_> { SpinSyncMutex::lock(self) }
  fn into_inner(self) -> T { SpinSyncMutex::into_inner(self) }
}
```

**理由**:

- 既存 caller (utils 内部の test 等) で `SpinSyncMutex` を直接使う箇所がある
- LockDriver trait import が無い文脈でも使える
- `RuntimeMutex<T, SpinSyncMutex<T>>` の内部 driver field として、inherent method 経由で動作

### 6. adapter driver は `utils-adaptor-std` に置く

std-only な driver impl (`DebugSpinSyncMutex`, 将来の `parking_lot` driver 等) は **`utils-adaptor-std` 配下に置く**。utils-core は **no_std 純度を維持**。

PR #1530 で feature = "std" を撤廃し、PR #1538 で utils-adaptor-std skeleton を確立した路線と整合する。

### 7. caller 移行戦略を crate 単位で分ける

**default type parameter を採用しない** ため、115 ファイルの caller は明示的に更新する必要がある。crate ごとの戦略を分ける:

| crate | 戦略 | 理由 |
|---|---|---|
| **actor-core/kernel** | **factory ジェネリック化** (`<F: LockDriverFactory>`, no default) | 本 change の主目的 (deadlock 検知) が actor-core 対象。test で `DebugSpinSyncFactory` に差し替える必要がある |
| **cluster-core** | **per-crate alias** (`KernelMutex<T> = RuntimeMutex<T, SpinSyncMutex<T>>`) | 本 change の主目的に含まれない。alias で互換維持し、必要が出たら別 change で factory 化 |
| **persistence-core** | 同上 (per-crate alias) | 同上 |
| **stream-core** | 同上 (per-crate alias) | 同上 |
| **actor-adaptor-std** | 同上 (per-crate alias、actor-core のジェネリック化には追従) | actor-core への呼び出しでは `SpinSyncFactory` を明示。内部 field では alias |
| **utils-core 内部** (wait/node 等) | **explicit** (`RuntimeMutex<T, SpinSyncMutex<T>>`) | alias を設けるほどの caller 数でもない。explicit のほうが意図が明確 |

per-crate alias はあくまで caller 側の **apparence 維持** のためのショートカットであり、crate の境界を越えて utils-core に逆流しない。alias ファイル自体は `modules/cluster-core/src/core/sync.rs` のような caller-crate-local な場所に置き、utils-core には置かない。

代替案:

- **A. 全 caller を factory ジェネリック化 (actor-core と同じ戦略)**: 本 change の scope が爆発する、却下
- **B. 全 caller で explicit (`RuntimeMutex<T, SpinSyncMutex<T>>`)**: 115 ファイルの文字列が冗長になる、却下
- **C. utils-core に `KernelMutex<T>` alias を置く**: utils-core が `SpinSyncMutex` を alias 経由で名指しする → 「port は adapter を知らない」原則違反、却下

### 8. `SpinSyncMutex` の配置は本 change では据え置く (将来判断を残す)

`SpinSyncMutex<T>` はアダプタ実装であるため、厳密な hexagonal では utils-core の外 (例えば `fraktor-utils-adaptor-spin-rs` 等の新 crate) に置くのが純粋。しかし本 change では:

- **port 定義ファイルの分離** (Decision 2) で実用上の純度は担保できる
- crate 分割は依存グラフ・Cargo.toml・feature 配線への影響が大きく、本 change の scope を爆発させる
- `SpinSyncMutex` は no_std default built-in driver として頻繁に使われており、caller の依存 crate を増やしたくない
- 将来 `parking_lot driver` などを追加するときに合わせて判断するほうが情報量が多い

ので **utils-core に据え置く**。ただし **port 定義ファイル (`runtime_mutex.rs`, `lock_driver.rs` 等) は `SpinSyncMutex` を一切参照しない** ことを厳守する。

代替案:

- **A. 本 change で `fraktor-utils-adaptor-spin-rs` に切り出す**: scope 爆発、却下
- **B. `SpinSyncMutex` を `utils-adaptor-std` に移動**: utils-adaptor-std は std-only のため no_std 制約と不整合、却下

## Risks / Trade-offs

- **Risk**: Phase 3 の actor-core/kernel ジェネリック化が ~30 ファイルに影響し、default factory がないため caller boundary (`ActorSystem::new` 等) ですべて `SpinSyncFactory` を明示する必要がある
  - **Mitigation**: caller boundary は限定的 (`ActorSystem::new` / test 用 factory / persistence 統合点など)。残りの kernel 内部は generic param `F` を propagate するだけ
- **Risk**: Phase 4 の per-crate alias 導入で各 crate に `sync.rs` 的な新規 / 既存ファイルへの追加が必要
  - **Mitigation**: alias は crate ごとに 2-3 行の型定義 + `pub use` のみ。bulk sed で field 型を置換可能
- **Risk**: GAT (Generic Associated Types) を使うため Rust 1.65 以降が必要
  - **Mitigation**: fraktor の nightly toolchain は遥かに新しい、問題なし
- **Risk**: 各 driver の実装で `Send` / `Sync` 制約が必要になる箇所がある
  - **Mitigation**: `LockDriver<T>: Sized` のみで開始し、`Send` / `Sync` は impl 側で個別に保証
- **Risk**: 本 change のレビューが大規模になる (~150 ファイル想定)
  - **Mitigation**: Phase ごとに commit を分け、独立に検証可能にする
- **Risk**: 115 ファイルの caller 書き換えでバグ混入の可能性
  - **Mitigation**: 書き換えは mechanical (sed / ast-grep) で済む。挙動変化なし (production は `SpinSyncMutex` 固定)。`cargo check --workspace --all-targets` + `cargo test` で検証
- **Trade-off**: API 表面が増える (`LockDriver`, `RwLockDriver`, `LockDriverFactory`, `RwLockDriverFactory`, `SpinSyncFactory` 等)
  - **Acceptance**: port 契約と adapter 注入の利益が上回る
- **Trade-off**: `Mailbox<SpinSyncFactory>` のような caller boundary での factory 明示が常に必要 (default なし)
  - **Acceptance**: hexagonal 純度のコストとして許容。caller boundary は限定的

## Migration Plan

各 phase は独立して `cargo check --workspace` clean になる状態を保つ。各 phase 内では必要な caller 書き換えを含め、phase 終了時に workspace が compile 通る状態で commit する。

### Phase 1: utils-core port contract introduction + utils-core 自身の caller 更新

- `lock_driver.rs` / `rwlock_driver.rs` 新設 (port trait, no adapter reference)
- `runtime_mutex.rs` / `runtime_rwlock.rs` 新設 (port struct, **no default type parameter**, no adapter reference)
- `lock_driver_factory.rs` 新設 (port trait, no adapter reference)
- `spin_sync_mutex.rs` / `spin_sync_rwlock.rs` に `impl LockDriver` / `impl RwLockDriver` 追加 (built-in driver 側に trait impl を置く)
- `spin_sync_factory.rs` / `spin_sync_rwlock_factory.rs` 新設 (built-in factory)
- 旧 `runtime_lock_alias.rs` の type alias を削除
- `sync.rs` の mod / pub use 配線更新
- **utils-core 自身の `RuntimeMutex<T>` 使用箇所 (wait/node_shared.rs 等) を `RuntimeMutex<T, SpinSyncMutex<T>>` に explicit 更新**
- 検証: `cargo check -p fraktor-utils-core-rs --lib --tests` clean

### Phase 2: utils-adaptor-std debug driver impls

- `DebugSpinSyncMutex<T>` / `DebugSpinSyncMutexGuard<T>` を再導入 (PR #1538 で OFF した実装を復活)
- `LockDriver<T>` impl を追加
- `DebugSpinSyncRwLock<T>` 新規追加 + `RwLockDriver<T>` impl
- `DebugSpinSyncFactory` / `DebugSpinSyncRwLockFactory` 新設
- unit tests 復活 (re-entry panic, contention, etc)
- `feature = "test-support"` で gate
- 検証: `cargo test -p fraktor-utils-adaptor-std-rs --features test-support`

### Phase 3: actor-core/kernel factory ジェネリック化

- 対象 shared 型の inventory: `Mailbox`, `ActorCell`, `MessageInvokerShared`, `MiddlewareShared`, `EventStreamShared`, `DeadLetterShared`, `SchedulerShared`, `SystemStateShared`, `SerializationRegistry`, etc (約 30 ファイル)
- 各型に `<F: LockDriverFactory>` (or `<R: RwLockDriverFactory>`) パラメータを追加 (**no default**)
- 内部 field の型を `RuntimeMutex<T, F::Driver<T>>` 等に書き換え
- 関連する `*Shared::new()` factory 関数も generic 化
- `ActorSystem` / `ActorContext` / `Props` 等の caller boundary で `SpinSyncFactory` を production path の型引数として固定
- 検証: `cargo test -p fraktor-actor-core-rs`

### Phase 4: 他 crate (cluster / persistence / stream / actor-adaptor-std) の per-crate alias 導入

- 各 crate に `KernelMutex<T>` / `KernelRwLock<T>` alias を導入 (例: `modules/cluster-core/src/core/sync.rs`)
  - `pub type KernelMutex<T> = fraktor_utils_core_rs::RuntimeMutex<T, fraktor_utils_core_rs::SpinSyncMutex<T>>;`
  - 同様に `KernelRwLock<T>`
- 各 crate の field 型 `RuntimeMutex<T>` / `RuntimeRwLock<T>` を `KernelMutex<T>` / `KernelRwLock<T>` に bulk 置換
- 各 crate から actor-core の type を呼ぶ箇所では `Mailbox<SpinSyncFactory>` 等を明示
- 検証: `cargo check --workspace --all-targets` clean

### Phase 5: actor-core test instrumentation example

- `modules/actor-core/Cargo.toml` の `[dev-dependencies]` に `fraktor-utils-adaptor-std-rs = { workspace = true, features = ["test-support"] }` を追加 (PR #1538 で削除したものを復活)
- `modules/actor-core/tests/deadlock_detection_example.rs` 新設:
  - `ActorSystem<DebugSpinSyncFactory>` で構築する test harness を提供
  - 意図的に再入する actor を用意し、`DebugSpinSyncFactory` 経由で panic することを `#[should_panic]` で verify
- 通常 contention は panic しないことも verify

### Phase 6: 検証 + spec delta + 命名規約のドキュメント更新

- `cargo check --workspace --all-targets` clean
- `./scripts/ci-check.sh ai all` exit 0
- `openspec validate lock-driver-port-adapter --strict` valid
- `.agents/rules/rust/immutability-policy.md` の AShared パターン記述に LockDriver の言及を追加 (任意)
- `modules/utils-core/src/core/sync/spin_sync_mutex.rs` の rustdoc を更新 (再入検知方法を `DebugSpinSyncFactory` 経由に書き換え)

## Open Questions

- **`RwLockDriver` の `WriteGuard` と `ReadGuard` の lifetime**: GAT で書けるが、`spin::RwLock` の guard 型と互換にする必要がある。実装時に詳細を詰める
- **`Send` / `Sync` の境界**: trait 定義に `Send` / `Sync` を含めるか? 含めないと caller 側で個別に bound する必要があるが、含めると no_send 型 (e.g., test mock) が impl できない。**含めない方針を初期値とする**
- **`LockDriverFactory::Driver<T>` の制約**: `LockDriver<T> + Send + Sync` 等を impose するか? **初期は `LockDriver<T>` のみ**
- **actor-core shared 型の pub 境界**: `Mailbox<F>` が pub なら F も pub になる。pub(crate) で隠すか、Sealed factory pattern で固定するか? **Sealed factory pattern を検討**
- **rwlock の Guard 型階層**: `ReadGuard` と `WriteGuard` が独立した GAT になる。生 `spin::RwLockReadGuard` と `spin::RwLockWriteGuard` を返すことになる
- **将来の `parking_lot` driver の追加余地**: 本 change の設計が parking_lot をきれいに受け入れられることを確認しておく
- **cluster-core / persistence-core / stream-core の factory ジェネリック化**: Phase 4 では per-crate alias で互換維持に留めたが、本当にそれでよいか? actor-core と同じ問題 (test 時に差し替え不可) を抱えるはず。後日の別 change 候補として記録
- **`SpinSyncMutex` の切り出し**: utils-core 内に据え置いたが、将来 `fraktor-utils-adaptor-spin-rs` 等の no_std adapter crate に切り出す価値があるか? parking_lot / std::sync 等の追加タイミングで再検討
