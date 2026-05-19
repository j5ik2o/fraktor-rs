## Context

`modules/utils` の同期プリミティブまわりは投機的に立てた抽象が積層しており、現状の構造は以下の通り:

```
呼び出し側コード (~150+ files)
       │
       ├──── RuntimeMutex<T>  (106 files) ─────┐
       │                                        │
       ├──── NoStdMutex<T>    (67 files)  ──────┤
       │                                        │
       └──── SpinSyncMutex<T> (44 files) ──┐    │
                                            │    │
                                            │    ▼
                                            │   runtime_lock_alias.rs
                                            │   (no-op type alias)
                                            │    │
                                            │    ▼
                                            │   lib.rs cfg switch
                                            │   feature="std"  → A
                                            │   !feature="std" → B
                                            │    │      │
                                            │    │      ▼
                                            │    │   compat shim
                                            │    │   pub(crate) type StdSyncMutex<T>
                                            │    │     = SpinSyncMutex<T>
                                            │    │      │
                                            │    │      ↓
                                            │    │   SpinSyncMutex
                                            │    ▼
                                            │   StdSyncMutex<T>
                                            │   (real std::sync::Mutex
                                            │    wrapper, 6 files,
                                            │    production caller 0)
                                            │    │
                                            │    │
                                            │    │ impl SyncMutexLike
                                            │    ▼
                                            └─→ SpinSyncMutex
                                                │
                                                │ impl SyncMutexLike
                                                ▼
                                            SyncMutexLike trait
                                            (generic-bound caller =
                                             SyncQueueShared<T,K,B,M>
                                             の M パラメータ)
                                                │
                                                │ M: SyncMutexLike<...>
                                                ▼
                                            SyncQueueShared family
                                            ├── SyncFifoQueueShared (alias)
                                            │     ↑ actor-core mailbox / stream-core stream_buffer で使用中
                                            ├── SyncMpscQueueShared (alias) ── caller ゼロ
                                            ├── SyncSpscQueueShared (alias) ── caller ゼロ
                                            ├── SyncPriorityQueueShared (alias) ── caller ゼロ
                                            ├── SyncMpscProducerShared/ConsumerShared ── caller ゼロ
                                            └── SyncSpscProducerShared/ConsumerShared ── caller ゼロ
```

複数の調査ドキュメント (`docs/plan/utils-plan.md`, `docs/plan/utils-plan-0.md`) で以下の問題が記録されている:

1. **抽象と実態の乖離**: `RuntimeMutex` 抽象に対して 44 ファイルが `SpinSyncMutex` を直接参照
2. **`SyncQueueShared` family の **部分的** デッドコード**: producer/consumer 単独型 4 つ + Mpsc/Spsc/Priority alias 3 つは workspace 内 caller ゼロ。一方で `SyncFifoQueueShared` (FIFO alias) と parent `SyncQueueShared` は actor-core mailbox と stream-core stream_buffer で使用中
3. **`SyncMutexLike` trait は `SyncQueueShared` の M パラメータ generic bound として生きている**: 単純な trait 削除は不可能。M パラメータの monomorphize が必要
4. **`SyncRwLockLike` trait の幽霊化**: 1 impl のみ、generic bound としての caller はゼロ。actor-core 7 ファイルは `.read()` / `.write()` のための trait import としてのみ使っている
5. **`StdSyncMutex` / `StdSyncRwLock` の死荷重**: production caller ゼロ、cfg switch の片方が無意味
6. **規約と clippy ルールと型定義の三方向不整合**: AShared パターン規約は `SpinSyncMutex` を指名、clippy は `SyncMutexLike` を指名、型定義は `RuntimeMutex` を提供

これは `mailbox-block-overflow-removal` change が解消した async backpressure scaffolding と同じ系統の design debt である:「投機的に抽象を立てた → 実需が出なかった → 各所が最短経路を選んだ → 規約が迂回側に寄せられた → 抽象が幽霊化し、死荷重として残った」

`utils-dead-code-removal` capability spec は既に存在し、`RcShared` / `AsyncMutexLike` 等を「公開 API から除外されていなければならない」と requirement にしている。本 change は同 capability の禁止リストを拡張する。

## Goals / Non-Goals

**Goals:**

- `SyncQueueShared` family の dead sub-types (producer/consumer 単独型 4 つ + Mpsc/Spsc/Priority alias 3 つ) を削除し、generic 境界の使用箇所を減らす
- `SyncQueueShared` の `M` 型パラメータを `SpinSyncMutex` に monomorphize し、`SyncMutexLike` trait の generic bound caller をゼロにする
- `StdSyncMutex` / `StdSyncRwLock` 系を削除し、`utils/src/std/` ディレクトリと `feature = "std"` を撤去する
- `SyncMutexLike` / `SyncRwLockLike` trait を削除し、1-impl trait の幽霊抽象を解消する
- `SpinSyncRwLock` に inherent `read()` / `write()` メソッドを追加し、AShared 系 7 ファイルが trait import なしで動くようにする
- `RuntimeMutex` / `RuntimeRwLock` / `NoStdMutex` alias は維持し、173 caller の touch を回避する
- `SyncQueueShared` / `SyncFifoQueueShared` は production 利用ありのため保持する
- 上記の削除を `utils-dead-code-removal` capability の禁止リスト拡張として spec delta に反映する

**Non-Goals:**

- `SyncQueueShared` / `SyncFifoQueueShared` 自体の削除 (production 利用ありのため保持)
- `SyncQueueShared` の `K` 型パラメータ (`FifoKey`/`MpscKey`/`SpscKey`/`PriorityKey`) の構造削除 (現在の production 用途は `FifoKey` のみだが、`K` の枠組み撤去はスコープ膨張)
- `RuntimeMutex` / `RuntimeRwLock` / `NoStdMutex` alias の rename or 削除 (cosmetic)
- `fraktor-utils-rs` の `*-core` / `*-adaptor-std` 分離 (削除後 adaptor 側に入れるものがゼロのため YAGNI)
- `modules/utils/src/core/` ディレクトリの flat 化 (cosmetic)
- DebugMutex (再入検出) の導入 (本 change の当初動機だったが scope out、別 change で扱う)
- `SharedAccess` trait の変更 (AShared 系の主流 API、無関係)
- `spin::Mutex` の挙動変更や `parking_lot` 等への切り替え (別問題)
- 規約 `.agents/rules/rust/immutability-policy.md` の変更 (既に `SpinSyncMutex` 指名済みで本 change と整合)

## Decisions

### 1. Option X (dead sub-types 削除 + `SyncQueueShared` の `M` パラメータ monomorphize) を採用する

調査時に検討した複数の選択肢のうち、**Option X** (dead sub-types を削除し、parent `SyncQueueShared` の `M` 型パラメータを `SpinSyncMutex` に固定して trait 削除を可能にする) を採用する。

| 選択肢 | scope | 機能効果 | 評価 |
|---|---|---|---|
| A: 完全一本化 | 150+ ファイル touch (RuntimeMutex → SpinSyncMutex の機械置換) | 同じ | 大量 mass rewrite で review 負荷高、bisect しにくい |
| B: trait のみ維持 (Std だけ消す) | 中 | 1-impl trait 残存 | 幽霊抽象が温存される |
| C: 現状維持 + 漏洩 cleanup | 44 ファイル (SpinSyncMutex → RuntimeMutex 機械置換) | 死荷重温存 | 根本不整合は解消されない |
| D: 全 Sync*Shared 削除 | 不可能 | — | `SyncFifoQueueShared` は production 利用ありのため削除不能 |
| **X: dead sub-types 削除 + monomorphize** | **~30 ファイル (削除中心 + actor-core mailbox.rs 1 行修正)** | **同じ** | **最小侵襲で trait 削除まで到達** |

代替案:

- **A (完全一本化)**: 150 ファイルを `RuntimeMutex` → `SpinSyncMutex` に機械置換する案。`RuntimeMutex` alias を残せば caller を touch する必要がない、という観察から却下
- **B (trait のみ維持)**: `Std*` は削除するが `SyncMutexLike` trait は残す案。1 impl の trait は YAGNI 違反。AShared 系の `use SyncRwLockLike` を消したいなら trait も削除する方が一貫
- **C (漏洩 cleanup のみ)**: 規約 (`SpinSyncMutex` 指名) と実装 (`RuntimeMutex` 主流) の不整合を逆方向に解消する案 (44 ファイルを `RuntimeMutex` に揃える)。死荷重を温存し、後続の DebugMutex 導入時に再度同じ判断を強いられる。却下
- **D (全 Sync*Shared 削除)**: 当初の探索段階で検討。`SyncFifoQueueShared` が actor-core mailbox.rs:115 と stream-core stream_buffer.rs:13 で使われていることを実コードで確認し却下

### 2. `SyncQueueShared` の `M` パラメータを monomorphize する (Option X の核心)

`SyncMutexLike` trait の唯一の generic bound caller は `SyncQueueShared<T, K, B, M: SyncMutexLike<...>>` の `M` パラメータである。trait を削除可能にするには `M` を取り除くしかない。

```rust
// Before
pub struct SyncQueueShared<T, K, B, M = SpinSyncMutex<SyncQueue<T, K, B>>>
where
  K: TypeKey,
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<SyncQueue<T, K, B>>,
{
  inner: ArcShared<M>,
  _pd:   PhantomData<(T, K, B)>,
}

// After
pub struct SyncQueueShared<T, K, B>
where
  K: TypeKey,
  B: SyncQueueBackend<T>,
{
  inner: ArcShared<SpinSyncMutex<SyncQueue<T, K, B>>>,
  _pd:   PhantomData<(T, K, B)>,
}
```

`SyncFifoQueueShared` alias も対応:

```rust
// Before
pub type SyncFifoQueueShared<T, B, M = SpinSyncMutex<SyncQueue<T, FifoKey, B>>> = SyncQueueShared<T, FifoKey, B, M>;

// After
pub type SyncFifoQueueShared<T, B> = SyncQueueShared<T, FifoKey, B>;
```

caller 側の影響:

- `actor-core/src/core/kernel/dispatch/mailbox.rs:115-116` の `UserQueueShared<T>` 型 alias から第 3 型パラメータ (`RuntimeMutex<...>`) を削除する必要あり (1 行差分)
- `actor-core/src/core/kernel/dispatch/mailbox/mailbox_queue_handles.rs:48-49` の construction site (`RuntimeMutex::new(sync_queue)` → `ArcShared::new(mutex)` → `UserQueueShared::<T>::new(...)`) は、`RuntimeMutex<T>` が `SpinSyncMutex<T>` の alias になっている (Phase 2 で monomorphize 済み) ため、無修正で動く
- `stream-core/src/core/impl/fusing/stream_buffer.rs:13` は既に 2 パラメータ形式 (`SyncFifoQueueShared<T, VecDequeBackend<T>>`) を使っているため無修正

代替案:

- **`K` パラメータも除去する**: `FifoKey` のみ生き残るので、理論上は `K` も hardcoded にできる。しかし `SyncQueue<T, K, B>` の K は基底レベルの型機構であり、これに踏み込むとスコープが膨張する。`M` 除去だけに絞る
- **`M` を残して trait のみ削除**: Rust では generic bound trait を削除しつつ generic param を残すことはできない (型推論ができなくなる)。技術的に不可能

### 3. dead Sync*Shared sub-types の削除範囲

以下を削除する:

- ファイル: `sync_mpsc_producer_shared.rs`, `sync_mpsc_consumer_shared.rs`, `sync_spsc_producer_shared.rs`, `sync_spsc_consumer_shared.rs` (+ tests dir)
- `sync_queue_shared.rs` 内の impl ブロック:
  - `impl<T, B, M> SyncQueueShared<T, MpscKey, B, M>` (`new_mpsc`, `producer_clone`, `into_mpsc_pair`)
  - `impl<T, B, M> SyncQueueShared<T, SpscKey, B, M>` (`new_spsc`, `into_spsc_pair`)
  - `impl<T, B, M> SyncQueueShared<T, PriorityKey, B, M>` (`peek_min`)
- `sync_queue_shared.rs` 内の type alias:
  - `pub type SyncMpscQueueShared = ...`
  - `pub type SyncSpscQueueShared = ...`
  - `pub type SyncPriorityQueueShared = ...`
- `queue.rs` の `mod` / `pub use` から該当エントリ削除
- `queue/tests.rs` の Mpsc/Spsc/Priority 関連テスト 削除 (FIFO テストは保持)

これらは workspace 内 caller ゼロを確認済み (`grep -rn` で `modules/utils/src/core/collections/queue/` 内部ファイルのみがヒット)。

代替案:

- **alias 3 つだけ削除して impl ブロックは残す**: 到達不可能なコードが残るため YAGNI 違反
- **`peek_min` の impl ブロックだけ残す**: PriorityKey は使われていないため到達不可能。残す意味がない

### 4. `SyncRwLockLike` trait 削除に伴い `SpinSyncRwLock` に inherent method を新設する

`SpinSyncMutex` には inherent な `lock()` メソッドが既に存在する一方で、`SpinSyncRwLock` には inherent な `read()` / `write()` が存在しない。AShared 系の actor-core 7 ファイルは `use ...SyncRwLockLike;` で trait method を可視化することで `.read()` / `.write()` を呼んでいる。

trait 削除を実現するには、`SpinSyncRwLock` に inherent な `read()` / `write()` を追加する必要がある。実装は `SpinSyncMutex::lock()` と同じ pattern (`self.0.read()` / `self.0.write()` への薄い委譲)。

代替案:

- **`SpinSyncRwLock` の `as_inner()` 経由で書き換える**: 7 ファイル全ての `.read()` / `.write()` を `.as_inner().read()` / `.as_inner().write()` に書き換える。inherent method を新設しない pure な抽象削除。ただし caller 側のコードがやや冗長になる。**却下**
- **trait を残す**: scope は最小だが、1-impl trait の幽霊抽象が温存される。**却下**

### 5. `clippy.toml` の `disallowed-types` replacement target を `SpinSyncMutex` に更新する

現状の 3 つの `clippy.toml` (utils, actor-core, cluster-core) は `std::sync::Mutex` の `replacement` target に `SyncMutexLike` を指定している。trait 削除に伴い、replacement target を `SpinSyncMutex` 直接参照に更新する。これにより:

- 規約 (`immutability-policy.md`: `SpinSyncMutex` 指名) と clippy 設定の方向性が一致する
- 開発者が新規コードを書く際の指針が明確になる (「`SpinSyncMutex` を使え」)
- `RuntimeMutex` / `NoStdMutex` 経由の使用も clippy 的には許容される (alias なので同じ型に解決される)

代替案:

- **`RuntimeMutex` を replacement target にする**: alias の方を指針にする。alias は機能的に意味があるが、規約と齟齬が出る。**却下**
- **clippy 設定を削除する**: `std::sync::Mutex` の禁止自体を緩める。安全性を下げるので **却下**

### 6. 段階的 PR 分割 (3 phase)

scope は大きくないが (~30 ファイル touch)、削除内容が独立しているので 3 phase に分けて bisect しやすくする。各 phase は独立してマージ可能。

```
Phase 1 (PR 1): dead Sync*Shared sub-types 削除
  - utils 内部のみ、caller ゼロ、最も低リスク
  - 4 ファイル削除 + sync_queue_shared.rs / queue.rs / tests.rs 整理

Phase 2 (PR 2): StdSyncMutex/RwLock + std mod + feature="std" 削除
  - utils 内部の構造変更 + adapter Cargo.toml の features 削除
  - caller 修正なし (alias chain で吸収)
  - ~10 ファイル削除/修正

Phase 3 (PR 3): SyncQueueShared monomorphize + SyncMutexLike/SyncRwLockLike trait 削除
  - sync_queue_shared.rs の M パラメータ除去
  - actor-core mailbox.rs の UserQueueShared 型 alias 修正 (1 行)
  - actor-core 7 ファイルの use 行削除
  - SpinSyncRwLock に inherent method 追加
  - clippy.toml 3 ファイルの replacement target 更新
  - rustdoc 参照更新
  - openspec spec delta
  - ~20 ファイル touch
```

代替案:

- **1 PR で全部やる**: ~30 ファイル touch を 1 PR でレビューすると負荷が高い。bisect もしにくい。**却下**
- **Phase 3 を分割する (monomorphize と trait 削除を別 PR にする)**: monomorphize は trait 削除の前提条件であり、独立にマージしても trait 削除が完了するまで一時的に整合性が崩れる (M パラメータ除去後も trait の impl が残るとビルドは通るが論理的に半端)。1 PR にまとめる方が筋がいい。**却下**

### 7. spec delta は MODIFIED Requirements で禁止リストを拡張する

既存の `utils-dead-code-removal` capability の `Requirement: 未使用の共有・同期補助型は公開 API に存在しない` に対して、本 change で削除する型を禁止リストに追加する形で MODIFIED Requirement を書く。新規 capability や新規 requirement は作らない。

`SyncQueueShared` および `SyncFifoQueueShared` は production で使用されているため、禁止リストには含めない (これらは保持する)。

理由:

- 既存 requirement と本 change の意図 (utils dead code 排除) が完全に一致する
- 新規 requirement を立てると重複した制約になる
- MODIFIED 形式は過去 change (`mailbox-block-overflow-removal` 等) と整合する

## Risks / Trade-offs

- **Risk**: `SyncQueueShared` の `M` パラメータ削除で actor-core mailbox.rs の型 alias が壊れる
  - **Mitigation**: 1 行の修正 (`SyncFifoQueueShared<T, VecDequeBackend<T>, RuntimeMutex<...>>` → `SyncFifoQueueShared<T, VecDequeBackend<T>>`)。compile error が即座に guide する
- **Risk**: `SyncQueueShared` の `M` パラメータ削除で construction site (`mailbox_queue_handles.rs:48-49`) の `ArcShared::new(mutex)` 周辺が壊れる
  - **Mitigation**: `RuntimeMutex<T>` は Phase 2 で `SpinSyncMutex<T>` の直接 alias に書き換えられる。construction site の `RuntimeMutex::new(sync_queue)` は `SpinSyncMutex::new(sync_queue)` と同義になり、`ArcShared<SpinSyncMutex<...>>` が構築される。`UserQueueShared::<T>::new` の signature とも一致するため無修正で動く
- **Risk**: workspace feature unification で actor-core が `RuntimeMutex` 経由で `StdSyncMutex` を引いている可能性
  - **Mitigation**: 本 change で `Std*` と `feature = "std"` を完全削除するので、unification 経路自体が消滅する。foot-gun 解消も兼ねる
- **Risk**: actor-core 7 ファイルの `use SyncRwLockLike` 削除で漏れがあると compile error
  - **Mitigation**: compile error が逐次 guide してくれる。手動で全件確認する必要なし
- **Risk**: `SpinSyncRwLock` に inherent method を追加することによる API 拡大
  - **Mitigation**: 純粋な追加であり、既存 trait method (`SyncRwLockLike::read/write`) の signature と一致する。後方互換的な変更
- **Risk**: clippy.toml の replacement target 更新で既存コードの clippy エラーが出る可能性
  - **Mitigation**: replacement target は警告メッセージで言及されるのみで、既存コードの compile に影響しない。ただし `cargo clippy` の出力が変わるので CI が通ることを確認
- **Risk**: `Cargo.toml` の `features = ["std"]` 削除で adapter 系の feature unification が変わる可能性
  - **Mitigation**: 削除する feature は `fraktor-utils-rs` の `std` feature であり、内容は std mod の存在のみ。std mod 自体を削除するので、削除しても何も壊れない。ただし `cargo build -p fraktor-utils-rs --no-default-features` 等の標準 build で動くことを確認
- **Trade-off**: alias を残すことで「`SpinSyncMutex` という canonical 名と `RuntimeMutex` という alias 名が併存する」状態が続く
  - **Mitigation**: 機能的には等価。命名統一は別 change (cosmetic) で扱う方が安全
- **Trade-off**: `SyncQueueShared` の `K` 型パラメータ (`FifoKey`/`MpscKey`/`SpscKey`/`PriorityKey`) を残すと、`MpscKey`/`SpscKey`/`PriorityKey` という未使用の型タグが残ってしまう
  - **Mitigation**: 本 change の scope ではこれらの type tag 自体は触らない。`SyncQueueShared` の K として使う impl ブロックは削除するため、実質的に到達不能。型タグの完全削除は別 cosmetic change で扱う

## Migration Plan

各 commit は独立して `cargo test -p fraktor-actor-core-rs --lib` 等のテストがグリーンになる状態を保つ。

### Commit 1: `feat(utils): delete dead Sync*Shared sub-types`

- 削除: `modules/utils/src/core/collections/queue/sync_mpsc_producer_shared.rs`
- 削除: `modules/utils/src/core/collections/queue/sync_mpsc_consumer_shared.rs`
- 削除: `modules/utils/src/core/collections/queue/sync_spsc_producer_shared.rs`
- 削除: `modules/utils/src/core/collections/queue/sync_spsc_consumer_shared.rs`
- 削除: `modules/utils/src/core/collections/queue/sync_spsc_producer_shared/tests.rs` とディレクトリ (存在する場合)
- 修正: `modules/utils/src/core/collections/queue/sync_queue_shared.rs`
  - `use super::{sync_mpsc_consumer_shared::SyncMpscConsumerShared, sync_mpsc_producer_shared::SyncMpscProducerShared, sync_spsc_consumer_shared::SyncSpscConsumerShared, sync_spsc_producer_shared::SyncSpscProducerShared};` を削除
  - `impl<T, B, M> SyncQueueShared<T, MpscKey, B, M>` ブロック (new_mpsc / producer_clone / into_mpsc_pair) 削除
  - `impl<T, B, M> SyncQueueShared<T, SpscKey, B, M>` ブロック (new_spsc / into_spsc_pair) 削除
  - `impl<T, B, M> SyncQueueShared<T, PriorityKey, B, M>` ブロック (peek_min) 削除
  - `pub type SyncMpscQueueShared = ...` 削除
  - `pub type SyncSpscQueueShared = ...` 削除
  - `pub type SyncPriorityQueueShared = ...` 削除
- 修正: `modules/utils/src/core/collections/queue.rs` から以下を削除
  - `mod sync_mpsc_consumer_shared;`
  - `mod sync_mpsc_producer_shared;`
  - `mod sync_spsc_consumer_shared;`
  - `mod sync_spsc_producer_shared;`
  - `pub use sync_mpsc_consumer_shared::SyncMpscConsumerShared;`
  - `pub use sync_mpsc_producer_shared::SyncMpscProducerShared;`
  - `pub use sync_spsc_consumer_shared::SyncSpscConsumerShared;`
  - `pub use sync_spsc_producer_shared::SyncSpscProducerShared;`
  - `pub use sync_queue_shared::{SyncMpscQueueShared, SyncPriorityQueueShared, SyncSpscQueueShared, ...};` から該当エントリのみ削除し、`SyncQueueShared, SyncFifoQueueShared` は残す
- 修正: `modules/utils/src/core/collections/queue/tests.rs`
  - `MpscKey` / `SpscKey` / `PriorityKey` を使うテスト関数 (`block_policy_reports_full`, `grow_policy_increases_capacity`, `priority_queue_supports_peek`, `mpsc_pair_supports_multiple_producers`, `spsc_pair_provides_split_access` 等) を削除
  - `FifoKey` を使うテスト (`offer_and_poll_fifo_queue`, `vec_ring_backend_provides_fifo_behavior` 等) は保持
  - `use super::{..., type_keys::{FifoKey, MpscKey, PriorityKey, SpscKey}, ...};` から不要な key を削除
- 検証: `cargo check -p fraktor-utils-rs --lib --tests` + `cargo test -p fraktor-utils-rs --lib`

### Commit 2: `refactor(utils): drop StdSyncMutex/RwLock and feature="std"`

- 削除: `modules/utils/src/std/sync_mutex.rs`
- 削除: `modules/utils/src/std/sync_mutex_guard.rs`
- 削除: `modules/utils/src/std/sync_rwlock.rs`
- 削除: `modules/utils/src/std/sync_rwlock_read_guard.rs`
- 削除: `modules/utils/src/std/sync_rwlock_write_guard.rs`
- 削除: `modules/utils/src/std.rs`
- 削除: 各 `tests` ディレクトリ配下の対応するテストファイル
- 修正: `modules/utils/src/lib.rs`
  - `#[cfg(feature = "std")] pub mod std;` 削除
  - `#[cfg(not(feature = "std"))] mod std { compat shim }` 削除
  - `pub(crate) type RuntimeMutexBackend<T> = std::StdSyncMutex<T>;` 削除
  - `pub(crate) type RuntimeRwLockBackend<T> = std::StdSyncRwLock<T>;` 削除
- 修正: `modules/utils/src/core/sync/runtime_lock_alias.rs`
  - `pub type RuntimeMutex<T> = RuntimeMutexBackend<T>;` を `pub type RuntimeMutex<T> = SpinSyncMutex<T>;` に書き換え
  - `pub type RuntimeRwLock<T> = RuntimeRwLockBackend<T>;` を `pub type RuntimeRwLock<T> = SpinSyncRwLock<T>;` に書き換え
  - `pub type NoStdMutex<T> = RuntimeMutex<T>;` は維持
  - `use crate::{RuntimeMutexBackend, RuntimeRwLockBackend};` を削除し、`SpinSyncMutex` / `SpinSyncRwLock` を import
- 修正: `modules/utils/Cargo.toml`
  - `[features]` から `std = [...]` 削除 (もし存在すれば)
  - `default-features` 等の他 feature の依存関係から `std` を外す
- 修正: `modules/actor-adaptor-std/Cargo.toml`
  - `fraktor-utils-rs = { workspace = true, features = ["alloc", "std", "unsize"] }` から `"std"` を削除
- 修正: `modules/cluster-adaptor-std/Cargo.toml` (同様)
- 修正: `modules/cluster-core/Cargo.toml` (同様)
- 修正: `modules/persistence-core/Cargo.toml`
  - `[features]` の `std = ["fraktor-utils-rs/std"]` を削除または別依存に張り替える
- 修正: `modules/remote-adaptor-std/Cargo.toml` (同様)
- 検証: `cargo check -p fraktor-utils-rs --lib --tests` + `cargo check -p fraktor-actor-core-rs --lib --tests` + `cargo test -p fraktor-actor-core-rs --lib`

### Commit 3: `refactor(utils): monomorphize SyncQueueShared and collapse SyncMutexLike/SyncRwLockLike`

#### 3.A SyncQueueShared monomorphize

- 修正: `modules/utils/src/core/collections/queue/sync_queue_shared.rs`
  - `use ...sync_mutex_like::{SpinSyncMutex, SyncMutexLike};` を `use ...sync_mutex_like::SpinSyncMutex;` に変更
  - `pub struct SyncQueueShared<T, K, B, M = SpinSyncMutex<SyncQueue<T, K, B>>>` から `M` パラメータ削除
  - `where M: SyncMutexLike<...>` 句を削除
  - `inner: ArcShared<M>` を `inner: ArcShared<SpinSyncMutex<SyncQueue<T, K, B>>>` に変更
  - `impl<T, K, B, M> SyncQueueShared<T, K, B, M>` の generic params から `M` 削除、`where M: SyncMutexLike<...>` 削除
  - `impl<T, B, M> SyncQueueShared<T, FifoKey, B, M>` 同様 (M 削除、where 句削除)
  - `pub type SyncFifoQueueShared<T, B, M = SpinSyncMutex<SyncQueue<T, FifoKey, B>>> = SyncQueueShared<T, FifoKey, B, M>;` を `pub type SyncFifoQueueShared<T, B> = SyncQueueShared<T, FifoKey, B>;` に変更
  - `shared(&self) -> &ArcShared<M>` の戻り値型を `&ArcShared<SpinSyncMutex<SyncQueue<T, K, B>>>` に変更
- 修正: `modules/actor-core/src/core/kernel/dispatch/mailbox.rs`
  - `pub(crate) type UserQueueShared<T> = SyncFifoQueueShared<T, VecDequeBackend<T>, RuntimeMutex<SyncQueue<T, FifoKey, VecDequeBackend<T>>>>;`
  - → `pub(crate) type UserQueueShared<T> = SyncFifoQueueShared<T, VecDequeBackend<T>>;`
  - 不要になった `use ...sync::RuntimeMutex;` を整理 (もし他で使われていなければ)
- 確認: `actor-core/src/core/kernel/dispatch/mailbox/mailbox_queue_handles.rs:48-49` の construction site が無修正で通ること (`RuntimeMutex::new(sync_queue)` は Phase 2 後 `SpinSyncMutex::new(sync_queue)` と等価)
- 確認: `stream-core/src/core/impl/fusing/stream_buffer.rs` は無修正で通ること

#### 3.B trait + impl 削除 + inherent method 追加 + 必須配線更新

- 削除: `modules/utils/src/core/sync/sync_mutex_like.rs`
- 削除: `modules/utils/src/core/sync/sync_rwlock_like.rs`
- 修正: `modules/utils/src/core/sync/sync_mutex_like/spin_sync_mutex.rs` から `impl SyncMutexLike for SpinSyncMutex` ブロックと `use ...SyncMutexLike;` 削除
- 修正: `modules/utils/src/core/sync/sync_rwlock_like/spin_sync_rwlock.rs` から `impl SyncRwLockLike for SpinSyncRwLock` ブロックと `use ...SyncRwLockLike;` 削除し、inherent な `pub fn read(&self) -> spin::RwLockReadGuard<'_, T>` / `pub fn write(&self) -> spin::RwLockWriteGuard<'_, T>` を追加
- 修正: `modules/utils/src/core/sync.rs` の `pub mod sync_mutex_like;` / `pub mod sync_rwlock_like;` 宣言を整理し、`SpinSyncMutex` / `SpinSyncRwLock` を facade 削除後も公開し続ける配線に更新
- (任意) ディレクトリのフラット化: `git mv` で `sync_mutex_like/spin_sync_mutex.rs` を `sync/spin_sync_mutex.rs` に、`sync_rwlock_like/spin_sync_rwlock.rs` を `sync/spin_sync_rwlock.rs` に移動。`sync_mutex_like/` / `sync_rwlock_like/` ディレクトリ削除

#### 3.C caller (`use` 行削除)

- 修正: `modules/actor-core/src/core/kernel/system/state/system_state_shared.rs` の `use ...sync_rwlock_like::SyncRwLockLike;` 削除
- 修正: `modules/actor-core/src/core/kernel/serialization/serialization_registry/registry.rs` (同様)
- 修正: `modules/actor-core/src/core/kernel/actor/actor_ref/dead_letter/dead_letter_shared.rs` (同様)
- 修正: `modules/actor-core/src/core/kernel/actor/scheduler/scheduler_shared.rs` (同様)
- 修正: `modules/actor-core/src/core/kernel/actor/messaging/message_invoker/invoker_shared.rs` (同様)
- 修正: `modules/actor-core/src/core/kernel/actor/messaging/message_invoker/middleware_shared.rs` (同様)
- 修正: `modules/actor-core/src/core/kernel/event/stream/event_stream_shared.rs` (同様)
- 修正: `modules/actor-core/src/core/kernel/system/state/system_state_shared/tests.rs` の `use ...SyncRwLockLike;` 削除
- 修正: `modules/utils/src/core/sync/runtime_lock_alias/tests.rs` の `use ...SyncRwLockLike;` / `cfg(not(feature = "std"))` ガード削除
- 修正: `modules/utils/src/core/sync/sync_mutex_like/spin_sync_mutex/tests.rs` の trait 経由 assertion を inherent method ベースに更新
- 修正: `modules/utils/src/core/sync/sync_rwlock_like/spin_sync_rwlock/tests.rs` の trait import を削除し、inherent method ベースのテストへ更新

#### 3.D clippy + rustdoc

- 修正: `modules/utils/clippy.toml` の `disallowed-types` で `std::sync::Mutex` の `replacement` を `fraktor_utils_rs::core::sync::SpinSyncMutex` (適切な path) に更新
- 修正: `modules/actor-core/clippy.toml` (同様)
- 修正: `modules/cluster-core/clippy.toml` (同様)
- 修正: `modules/actor-core/src/core/typed/dsl/timer_scheduler.rs` の rustdoc 参照を `SpinSyncMutex` に更新
- 修正: `modules/utils/src/core/sync/shared_access.rs` の rustdoc 参照を `SpinSyncMutex` に更新

#### 3.E openspec spec delta

- 修正: `openspec/changes/utils-sync-collapse/specs/utils-dead-code-removal/spec.md` を MODIFIED Requirements 形式で確定
- 検証: `openspec validate utils-sync-collapse --strict`

#### 3.F 検証

- `cargo check -p fraktor-utils-rs --lib --tests`
- `cargo check -p fraktor-actor-core-rs --lib --tests`
- `cargo check -p fraktor-actor-adaptor-rs --lib --tests --features tokio-executor`
- `cargo test -p fraktor-utils-rs --lib`
- `cargo test -p fraktor-actor-core-rs --lib`
- `./scripts/ci-check.sh ai dylint`
- `grep -rn "SyncMutexLike\|SyncRwLockLike" modules/` がヒット 0 を返す (clippy.toml 内の replacement target 言及を除く)

### 最終検証

- [ ] `./scripts/ci-check.sh ai all` exit 0
- [ ] `openspec validate utils-sync-collapse --strict` valid
- [ ] `grep -rn "SyncMpscQueueShared\|SyncSpscQueueShared\|SyncPriorityQueueShared\|SyncMpscProducerShared\|SyncMpscConsumerShared\|SyncSpscProducerShared\|SyncSpscConsumerShared" modules/` がヒット 0
- [ ] `grep -rn "StdSyncMutex\|StdSyncRwLock\|StdMutex\|RuntimeMutexBackend\|RuntimeRwLockBackend" modules/` がヒット 0
- [ ] `grep -rn "SyncMutexLike\|SyncRwLockLike" modules/` がヒット 0 (clippy.toml の replacement 内言及は除く)
- [ ] `RuntimeMutex` / `NoStdMutex` / `RuntimeRwLock` の caller (合計 173) が無修正のまま動作
- [ ] `SyncQueueShared` / `SyncFifoQueueShared` の caller (actor-core mailbox + stream-core stream_buffer) が動作

## Open Questions

- **`fraktor-utils-rs` を crates.io で公開する想定があるなら、本 change は外部利用者にとって BREAKING change になる**:
  - 削除する公開シンボル (dead Sync*Shared sub-types / `StdSyncMutex` / `SyncMutexLike` 等) を import している外部 crate が万一存在するとビルドが壊れる
  - プロジェクトは「リリース前開発フェーズ」で「後方互換不要」と CLAUDE.md に明記されているので、原則 BREAKING を許容
  - ただし changelog (CHANGELOG.md は github action 自動生成のため AI は触らない) で言及される可能性がある
- **`feature = "std"` を完全削除するタイミング**:
  - Phase 2 で `fraktor-utils-rs/std` を削除するなら、adapter 系 Cargo.toml だけでなく `persistence-core` の `std = ["fraktor-utils-rs/std"]` 依存も同時に整理する必要がある
  - feature 名の削除を先行させると manifest 解決自体が壊れる可能性があるため、関連 Cargo.toml は同 commit で更新するのが安全
- **ディレクトリのフラット化を Phase 3 に含めるか**:
  - `sync_mutex_like/spin_sync_mutex.rs` を `sync/spin_sync_mutex.rs` に移動するのは git mv による履歴保持が望ましいが、必須ではない
  - 移動しなくても `mod` パス名 (`sync_mutex_like::SpinSyncMutex`) は trait 名に由来する不自然な命名のまま残る
  - Phase 3 で同時にやる方がきれい。Phase 3 のスコープ膨張が許容できれば実施
- **`SyncQueueShared` の `K` パラメータ (FifoKey/MpscKey/SpscKey/PriorityKey) の今後**:
  - 本 change 後、production で実際に使われるのは `FifoKey` のみになる
  - `K` パラメータ自体の削除や `MpscKey`/`SpscKey`/`PriorityKey` 型タグの削除は別 cosmetic change で扱うべきか?
  - 現状の判断: スコープ膨張のため本 change では触らない
