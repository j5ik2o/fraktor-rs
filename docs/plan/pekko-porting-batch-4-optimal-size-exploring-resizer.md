# Batch 4: `OptimalSizeExploringResizer` 移植

## 概要

pekko-porting ワークフローの Batch 4 は、Phase 3 hard の
`OptimalSizeExploringResizer`（Pekko classic / typed 双方）を fraktor-rs の typed
routing DSL へ移植するバッチである。

Pekko は classic routing 側に `DefaultOptimalSizeExploringResizer` を提供し、
typed 側の `PoolRouter::withResizer` から `Resizer` 経由で利用する構造を取る。
fraktor-rs では `Resizer` trait が typed routing DSL (`core/typed/dsl/routing/`)
に集約されているため、本バッチは **typed 側単独に新設** する。

本ドキュメントは `docs/gap-analysis/actor-gap-analysis.md`（第 7 版）からリンクされる
判定根拠ドキュメントとして機能する。

## 参照資料

| 参照対象 | パス |
|----------|------|
| Pekko `OptimalSizeExploringResizer` | `references/pekko/actor/src/main/scala/org/apache/pekko/routing/OptimalSizeExploringResizer.scala` |
| Pekko `DefaultResizer`（旧系列） | `references/pekko/actor/src/main/scala/org/apache/pekko/routing/Resizer.scala` |
| fraktor-rs `Resizer` trait | `modules/actor-core/src/core/typed/dsl/routing/resizer.rs` |
| fraktor-rs `DefaultResizer` | `modules/actor-core/src/core/typed/dsl/routing/default_resizer.rs` |
| fraktor-rs `PoolRouter` | `modules/actor-core/src/core/typed/dsl/routing/pool_router.rs` |
| 本バッチ主型 | `modules/actor-core/src/core/typed/dsl/routing/optimal_size_exploring_resizer.rs` |
| Clock DI 先行事例 | `modules/actor-core/src/core/kernel/pattern/{clock.rs,circuit_breaker.rs}` |
| Mailbox 観測先行事例 | `modules/actor-core/src/core/kernel/routing/smallest_mailbox_routing_logic.rs` |
| 設計ルール | `.agents/rules/rust/{immutability-policy,reference-implementation,naming-conventions,type-organization,cqs-principle}.md` |

## Pekko の契約意図（ユーザー可視）

Pekko `DefaultOptimalSizeExploringResizer` がユーザーに保証する契約は以下の 5 点。

1. **自動最適化**: ユーザーは `lowerBound` / `upperBound` と少数の探索パラメータを指定するだけ。
   実行時にスループット（単位メッセージあたり処理時間）を観測し、プールサイズを自動調整する。
2. **3 アクション周期**: `actionInterval` を経過するごとに `downsize` / `explore` / `optimize`
   のいずれか 1 つを選択実行する。
3. **フル稼働時のみメトリクス蓄積**: 全 routee が work 中（mailbox 非空）の時だけ
   `performanceLog` を更新し、アイドル時の計測歪みを避ける。
4. **長期未活用時の縮小**: `downsizeAfterUnderutilizedFor` 期間フル稼働していなかった場合、
   その期間中に観測した最大稼働数 × `downsizeRatio` までプールを縮小する。
5. **境界遵守**: 最終結果は `[lowerBound, upperBound]` にクランプされる。

これら 5 点が「契約意図」であり、Rust で再表現できていれば Pekko 互換の目的は達成される。
実装構造（Scala の trait 階層 / `var` ベースの mutation / `ThreadLocalRandom` / `LocalDateTime`）を
そのまま模倣することは目的ではない（`.agents/rules/rust/reference-implementation.md` の「最小 API」方針）。

## Pekko 側の内部実装要素

Pekko の `DefaultOptimalSizeExploringResizer` が上記契約を実現するために内部で採用している要素:

| 要素 | 役割 |
|------|------|
| `trait OptimalSizeExploringResizer extends Resizer` | `Resizer` に `reportMessageCount` を追加 |
| `DefaultOptimalSizeExploringResizer` case class（10 params） | 確率的 hill climbing + random exploration のパラメータ束 |
| `UnderUtilizationStreak(start, highestUtilization)` | フル稼働していない連続期間の記録 |
| `ResizeRecord(underutilizationStreak, messageCount, totalQueueLength, checkTime)` | 前回計測時点のスナップショット |
| `PerformanceLog = Map[PoolSize, Duration]` | プールサイズ → 1 メッセージあたり処理時間 |
| `var performanceLog` / `var record` / `var stopExploring` | 内部ミューテーション状態 |
| `isTimeForResize(messageCounter): Boolean` | `System.nanoTime() > record.checkTime + actionInternalNanos` の経過時間判定 |
| `reportMessageCount(currentRoutees, messageCounter)` | 各 routee の mailbox length を観測、指数減衰平均で `performanceLog` 更新 |
| `resize(currentRoutees): Int` | 3 分岐: downsize / explore / optimize |
| `explore(currentSize)` | `random.nextInt(ceil(currentSize * exploreStepSize))` による ±change |
| `optimize(currentSize)` | 隣接サイズから最小 Duration サイズへ半歩移動（ceil / floor） |
| `PoolSize = Int` type alias | 可読性のみ |
| `@SerialVersionUID(1L)` | JVM シリアライズ用 |
| `apply(resizerCfg: Config)` | HOCON ファクトリ |

これらは「契約意図」ではなく「JVM / Scala で案件を実現するときの実装詳細」である。
Rust / no_std / `immutability-policy.md` 準拠の下ではいくつかは採用不能、または別の形で
翻訳する必要がある。

## fraktor-rs 現行基盤との突合

| 観点 | Pekko | fraktor-rs（Batch 4 前） |
|------|-------|-----------|
| `Resizer` trait の置き場 | classic + typed 両方 | typed のみ (`core/typed/dsl/routing/resizer.rs`) |
| `Resizer::resize` 署名 | `resize(currentRoutees: IndexedSeq[Routee]): Int` | `resize(&self, count: usize) -> i32` |
| メトリクス観測 entry | `reportMessageCount(routees, counter)` | 不在 |
| 経過時間 API | `System.nanoTime()` 直接参照 | `Clock` trait（`CircuitBreaker<C: Clock>` で先行事例） |
| 乱数 | `ThreadLocalRandom` | LCG 先行事例（`group_router.rs:211` / `pool_router.rs:326` の `pseudo_random_index`） |
| mailbox 観測 | `Routee.asInstanceOf[ActorRefRoutee]` downcast | `ActorRef::system_state()?.cell(&pid)?.mailbox().user_len()`（`smallest_mailbox_routing_logic::observe_actor_ref` 先行事例） |
| 内部状態 | `var` 3 本 | `SpinSyncMutex<T>`（AShared パターン） |
| 公開型数ポリシー | trait + case class + 内部型 2 + HOCON factory | `reference-implementation.md` に従い最小数 |

## 判定（Pekko 要素 → fraktor-rs 採用方針）

| Pekko 要素 | 判定 | 根拠 |
|------------|------|------|
| `OptimalSizeExploringResizer` の契約意図（自動最適化）| **採用** | 契約 1〜5 を Rust で再表現 |
| `trait OptimalSizeExploringResizer extends Resizer` の 2 段構成 | **翻訳（1 段に集約）** | Scala では trait extraction に意味があるが、fraktor-rs では実装 1 件のみのため単一 struct にまとめる。公開型は `OptimalSizeExploringResizer` のみ |
| 10 個のチューニングパラメータ | **採用（全部）** | 契約 1〜5 の挙動を決める公開パラメータすべて。`numOfAdjacentSizesToConsiderDuringOptimization` は Pekko 自身が HOCON key を `optimization-range` と短縮しているため Rust 側も `optimization_range: usize` を採用 |
| デフォルト値 10 件（`lowerBound=1`, `upperBound=30`, `chanceOfScalingDownWhenFull=0.2`, `actionInterval=5s`, `optimization-range=16`, `exploreStepSize=0.1`, `downsizeRatio=0.8`, `downsizeAfterUnderutilizedFor=72h`, `explorationProbability=0.4`, `weightOfLatestMetric=0.5`）| **採用（そのまま）** | Pekko 実装が長年運用してきた閾値セット。独自調整はエビデンスなしに避ける |
| パラメータバリデーション（`lowerBound > 0`、`upperBound >= lowerBound`、確率 ∈ `[0,1]`、`optimization-range >= 2` など）| **翻訳** | Pekko は `IllegalArgumentException` を throw。Rust では `assert!` で panic（`DefaultResizer::new` の先行事例に揃える） |
| `reportMessageCount` メソッドの追加 | **翻訳（default no-op）** | Pekko はサブトレイト追加だが、Rust では `Resizer` trait 本体に `fn report_message_count(&self, _mailbox_sizes: &[usize], _counter: u64) {}` を default 実装付きで追加。`DefaultResizer` は default を継承するため挙動不変 |
| `isTimeForResize(messageCounter)` 署名 | **翻訳** | 現行 `is_time_for_resize(&self, u64) -> bool` を維持。`message_counter` は `DefaultResizer` 側で必要なため署名を揃える。本 resizer は内部で `Clock` から経過時間を取って判定 |
| `resize(currentRoutees: IndexedSeq[Routee])` 署名 | **翻訳（破壊的変更）** | 現行 `resize(&self, usize) -> i32` を `resize(&self, mailbox_sizes: &[usize]) -> i32` に変更。`DefaultResizer` は `.len()` のみ使うため挙動不変。`OptimalSizeExploringResizer` は全要素を利用。後方互換不要方針で容認 |
| `LocalDateTime.now` / `System.nanoTime()` | **翻訳（Clock DI）** | `OptimalSizeExploringResizer<C: Clock>` でジェネリクス化（`CircuitBreaker<C: Clock>` に揃える）。`Clock::now()` と `Clock::elapsed_since(Instant)` を用いて経過時間を算出 |
| `checkTime = 0L` sentinel（Pekko は nanoTime が必ず正である前提で sentinel として 0 を使う）| **翻訳** | `Clock::Instant` は抽象型で「0」に相当する値が自明でないため、`ResizeRecord` に `has_recorded: bool` を追加し `check_time` は初期化時に `clock.now()` を入れる。Pekko の `checkTime > 0` ガードは `has_recorded && under_utilization_streak.is_none()` に対応 |
| `ThreadLocalRandom` | **翻訳（seedable LCG）** | `pool_router.rs:326` / `group_router.rs:211` で既に使っている `6_364_136_223_846_793_005`/`1_442_695_040_888_963_407`（Numerical Recipes MMIX）定数の LCG を `Lcg` 型に切り出し、`OptimalSizeExploringResizer::new(.., seed: u64)` で seed 指定可能にする。決定的なテストが可能 |
| `var performanceLog` / `var record` / `var stopExploring` | **翻訳（AShared）** | `SpinSyncMutex<State<I>>` を struct に持ち、`Resizer::resize` / `report_message_count` は `&self` 契約を維持（trait 契約順守）。`stopExploring` は Pekko でも `private[routing]` で外部非公開かつ初期値 `false` のまま本 Batch では利用しないため **フィールド自体を持たない**（YAGNI） |
| `Routee::ActorRefRoutee` downcast による mailbox 取得 | **非採用** | 型安全性が損なわれる。代わりに `PoolRouter` 側で `observe_routee_mailbox_sizes` helper（`ActorRef::system_state()?.cell(&pid)?.mailbox().user_len()`）を構築し、`&[usize]` に抽象化した値を `Resizer` に渡す |
| `PerformanceLog = Map[PoolSize, Duration]` type alias | **翻訳（型そのまま、alias は非公開）** | `alloc::collections::BTreeMap<usize, core::time::Duration>` を `State` 内部に保持。公開 type alias は提供しない（`HashMap` と違い `BTreeMap` を選んだ理由は no_std 対応と `filter { size >= left && size <= right }` が ordered walk で書ける点） |
| `UnderUtilizationStreak` | **翻訳** | `pub(crate) struct UnderUtilizationStreak<I> { start: I, highest_utilization: usize }` として独立ファイルに配置（`type-organization.md` 条件 c 不充足のため独立）。`pub(crate)` に留めるのは外部公開すべき API ではないため |
| `ResizeRecord` | **翻訳** | 同上、独立ファイル。Pekko の 4 フィールド + `has_recorded: bool` の 5 フィールド |
| `Lcg`（seedable RNG） | **新設（独立ファイル）** | `pool_router.rs` / `group_router.rs` の LCG ロジックを Batch 4 で本 resizer 用に切り出し。将来 resizer 以外でも共通化する余地はあるが、本バッチでは `optimal_size_exploring_resizer/` 配下の private ユーティリティに留める（YAGNI） |
| `State<I>` struct | **新設（独立ファイル）** | `SpinSyncMutex<State<I>>` で保護する mutable bookkeeping。`performance_log` / `record` / `rng` を束ねる。`pub(crate)` で親モジュールからのみアクセス |
| `PoolSize = Int` type alias | **非採用** | Rust では `usize` 直接使用で可読性十分 |
| `@SerialVersionUID(1L)` | **非採用** | JVM シリアライズ固有 |
| `apply(resizerCfg: Config)` HOCON ファクトリ | **非採用** | fraktor-rs は typed DSL の builder パターンで設定を受ける。HOCON は存在しない |
| `stopExploring` の public setter | **非採用（YAGNI）** | Pekko でも `private[routing]` で外部非公開、かつ実装内でも `checkTime = 0L` ブランチの内側でしか使われていない。本バッチでは `reportMessageCount` 側の「サンプリング停止」は `has_recorded` の更新停止では表現せず、**将来拡張が必要になった時点で追加**する |
| kernel 側への `Resizer` trait 移設 | **非採用** | Pekko の classic/typed 2 段構成は Scala の history 的理由（classic → typed の移行期）に由来。fraktor-rs では typed に集約済み。kernel 側に 1 件だけ trait を追加すると「Pekko に似せる目的化」に該当。gap-analysis の "core/kernel" ラベルは Pekko 側レイヤ表記であり fraktor-rs 側の移植先を縛らない |

## fraktor-rs での実装要素

### ファイル構成

```
modules/actor-core/src/core/typed/dsl/routing/
├── resizer.rs                                 # trait 拡張（署名変更 + report_message_count 追加）
├── default_resizer.rs                         # 新 resize 署名への追従（挙動不変）
├── default_resizer/tests.rs                   # 署名追従
├── pool_router.rs                             # observe_routee_mailbox_sizes helper + 配線
├── pool_router/tests.rs                       # smoke test（resizer 装着確認）
├── routing.rs                                 # mod + re-export
└── optimal_size_exploring_resizer.rs          # 新規・主型（本体 350 行）
    ├── lcg.rs                                 # 新規・seedable LCG（46 行）
    ├── resize_record.rs                       # 新規・pub(crate) struct（30 行）
    ├── state.rs                               # 新規・pub(crate) struct（21 行）
    ├── under_utilization_streak.rs            # 新規・pub(crate) struct（14 行）
    └── tests.rs                               # write_tests ステップで 15 ケース + smoke
```

### 型配置の判定（type-organization.md 適用）

| 型 | `pub` 範囲 | 配置 | 判定 |
|----|-----------|------|------|
| `OptimalSizeExploringResizer<C>` | `pub` | `optimal_size_exploring_resizer.rs`（主型） | 主型 |
| `ResizeRecord<I>` | `pub(crate)` | `resize_record.rs` | 親型フィールド。独立配置で `type-per-file` lint 整合 |
| `UnderUtilizationStreak<I>` | `pub(crate)` | `under_utilization_streak.rs` | 同上 |
| `State<I>` | `pub(crate)` | `state.rs` | 同上 |
| `Lcg` | `pub(crate)` | `lcg.rs` | ユーティリティ。将来の共通化を見越して独立配置 |

`pub(crate)` の型群は **公開型ではないため** `type-per-file-lint` の義務対象外だが、
将来の共通化・差し替え・テスト容易性のためすべて独立ファイル配置とした。主型ファイルは
アルゴリズム本体 + `Resizer` impl + private helper（`explore` / `optimize` / `libm_ceil` / `libm_floor`）に
責務を限定し、約 350 行で維持する。

### `Resizer` trait の署名変更

```rust
pub trait Resizer: Send + Sync {
  fn is_time_for_resize(&self, message_counter: u64) -> bool;
  fn resize(&self, mailbox_sizes: &[usize]) -> i32;  // ← usize → &[usize]
  fn report_message_count(&self, _mailbox_sizes: &[usize], _message_counter: u64) {}  // ← 新設、default no-op
}
```

- `resize` は破壊的変更（`usize` → `&[usize]`）。`DefaultResizer` は `.len()` のみ使うため挙動不変。
- `report_message_count` は default no-op で追加。`DefaultResizer` は override しないため影響ゼロ。

### `PoolRouter` の配線変更

```rust
// pool_router.rs 内のメッセージ受信側
let mailbox_sizes =
  routees_for_msg.with_lock(|routees| observe_routee_mailbox_sizes(routees.as_slice()));
resizer.report_message_count(&mailbox_sizes, counter);
if resizer.is_time_for_resize(counter) {
  let delta = resizer.resize(&mailbox_sizes);
  ...
}
```

- 同一メッセージ内で `report_message_count` と `resize` が同じ mailbox snapshot を共有する
  ことで、Pekko `ResizablePoolCell.sendMessage` が `preSendMessage` → `handleMessage` の間で
  observation を持ち越すロジックと挙動を合わせる。
- `report_message_count` は **毎メッセージ呼ぶ**（Pekko 準拠）。default no-op のため
  `DefaultResizer` ユーザーへの性能影響は関数呼び出し 1 回ぶん（インライン化想定）。
- `observe_routee_mailbox_sizes` は `smallest_mailbox_routing_logic::observe_actor_ref`
  のロジックを流用。mailbox が取れない（routee が既に停止している等）場合は `0` で埋める。
  unreachable routee には traffic を呼び込まないのが Pekko の挙動であり、Pekko-parity。

### 内部状態と AShared パターン

`OptimalSizeExploringResizer<C>` 本体は immutable な config（10 params + `clock` + `state` mutex）を
`&self` メソッドで提供する。mutable state は `SpinSyncMutex<State<C::Instant>>` に格納し、
`Resizer::resize` / `report_message_count` の内部でのみ `lock()` する。

これは `immutability-policy.md` の「薄い同期ラッパーは `*Shared`」規約の軽微な逸脱に該当するが:

- `DefaultResizer`（同一 trait の既存実装）が stateless `&self` で公開されており、
  `OptimalSizeExploringResizerShared` のような 2 段構成にするとユーザー面の整合性が崩れる
- `Resizer` trait 契約が `&self` を要求しており、`OptimalSizeExploringResizerShared` を作っても
  `Resizer for OptimalSizeExploringResizerShared` に委譲するだけで API としては等価
- `SpinSyncMutex` は struct 内 field として閉じ込められ、外部にロックを返さない
- `CircuitBreaker<C: Clock>` も同系の「内部 mutex + `&self` メソッド」構造で先行事例がある

以上の理由から **単一 struct + 内部 SpinSyncMutex** を採用する（Pekko `DefaultResizer` とも整合）。

### sentinel 置換

Pekko の `checkTime = 0L` sentinel は JVM `System.nanoTime()` が必ず正値である前提に依存する。
fraktor-rs の `Clock::Instant` は抽象型（associated type）で、`0` に相当する値が自明でない。
そのため `ResizeRecord` に `has_recorded: bool` を追加し、`check_time` は初期化時に
`clock.now()` を入れて `has_recorded = false` から始める。Pekko の `checkTime > 0` ガードは
`has_recorded && under_utilization_streak.is_none()` に対応する。

### `libm_ceil` / `libm_floor` のローカル実装

`core::f64::ceil` / `floor` は `std` を要求する。本 crate は `no_std` を維持するため、主型ファイル末尾に
整数キャスト経由の簡易実装を private helper として持つ。範囲は `usize * f64` でしか使われず、
64bit 整数にキャストできる領域に限定されているため、`truncated = x as i64 as f64` → 境界調整で十分。

## 契約 1〜5 と実装の対応

| 契約 | 実装位置 | 検証テスト（`optimal_size_exploring_resizer/tests.rs`） |
|------|----------|--------------------------------------------------------|
| 1 自動最適化 | `Resizer for OptimalSizeExploringResizer` 全体 | 15 テストが全体を総合的に検証 |
| 2 3 アクション周期 | `Resizer::is_time_for_resize` / `resize` の分岐 | `is_time_for_resize_respects_action_interval`、`explore_stays_positive_most_of_the_time`、`explore_goes_negative_when_chance_is_one`、`optimize_moves_half_way_toward_best_size` |
| 3 フル稼働時のみメトリクス | `report_message_count` の `fully_utilized && !streak.is_some() && has_recorded` ガード | `report_message_count_ignores_non_fully_utilized_samples`、`report_message_count_updates_performance_log_on_fully_utilized` |
| 4 長期未活用時の縮小 | `resize` の `expired_streak` ブランチ（`streak.highest_utilization as f64 * downsize_ratio`） | `resize_downsizes_after_underutilized_period` |
| 5 境界遵守 | `resize` 末尾の `clamped = lower.max(...).min(upper)` | `resize_respects_lower_bound`、`resize_respects_upper_bound` |
| バリデーション 5 件 | `new` / `with_*` の `assert!` | `new_rejects_zero_lower_bound`、`new_rejects_upper_below_lower`、`with_exploration_probability_rejects_out_of_range`、`with_optimization_range_rejects_below_two`、`with_weight_of_latest_metric_rejects_out_of_range` |
| short-circuit | `resize` の `perf_log.is_empty() && !streak.is_some()` ブランチ | `resize_noop_when_performance_log_empty_and_not_underutilized` |

## 設計ルールとの整合

| ルール | 整合状況 |
|--------|----------|
| `.agents/rules/rust/immutability-policy.md` | `SpinSyncMutex` を field に閉じ込め `&self` 契約維持。`*Shared` 命名ではなく単一 struct としたのは `DefaultResizer` / `CircuitBreaker` の先行事例との整合優先。ロック区間はメソッド内に閉じる（ガードを返さない） ✅ |
| `.agents/rules/rust/reference-implementation.md` | Scala trait 2 段 → Rust 単一 struct、`var` → `SpinSyncMutex`、`System.nanoTime` → `Clock` DI、`ThreadLocalRandom` → seedable LCG、HOCON → builder。型数は Pekko（trait + case class + 2 内部型 = 4）に対し本実装（主型 + State + ResizeRecord + UnderUtilizationStreak + Lcg = 5）で 1.5 倍以下 ✅ |
| `.agents/rules/rust/naming-conventions.md` | rustdoc 英語 / Markdown 日本語 ✅。`*Resizer` サフィックスは Pekko 由来のドメイン用語で `naming-conventions.md` 例外節（「参照実装の命名優先」）に該当 |
| `.agents/rules/rust/type-organization.md` | `pub(crate)` 型群は lint 対象外だが独立配置。公開型は `OptimalSizeExploringResizer` 1 件のみ ✅ |
| `.agents/rules/rust/cqs-principle.md` | `is_time_for_resize` / `resize` は Query（内部 lock は実装詳細）。`report_message_count` は Command（`()` 返却）。CQS 違反なし ✅ |
| `CLAUDE.md` | 後方互換不要方針で `resize` の破壊的変更容認。計画ドキュメントは `docs/plan/` 配下（本文書）。TAKT ムーブメント中は `final-ci` 以外で `ci-check.sh ai all` を走らせない ✅ |
| `.agents/rules/ignored-return-values.md` | `assert!` で return 相当の validation。`Resizer::resize` / `report_message_count` の戻り値は呼び出し元 `pool_router.rs` で `delta` / `()` として扱われ握りつぶしなし ✅ |

## 採用した判定結果

| Pekko 要素 | 判定 | 実装位置 |
|------------|------|----------|
| `DefaultOptimalSizeExploringResizer` 契約 1〜5 | **採用（翻訳）** | `optimal_size_exploring_resizer.rs` 全体 |
| 10 チューニングパラメータ + デフォルト値 | **採用（そのまま）** | `OptimalSizeExploringResizer<C>` フィールド + `new` のデフォルト |
| パラメータバリデーション | **採用（`assert!` 化）** | `new` / `with_*` メソッド |
| `UnderUtilizationStreak` / `ResizeRecord` / `PerformanceLog` | **採用（翻訳）** | `under_utilization_streak.rs` / `resize_record.rs` / `state.rs` 内 BTreeMap |
| `reportMessageCount` trait メソッド | **採用（default no-op 形式）** | `resizer.rs` |
| `resize(IndexedSeq[Routee])` → `resize(&[usize])` | **翻訳（破壊的変更）** | `resizer.rs` + `default_resizer.rs` + `pool_router.rs` |
| `isTimeForResize(messageCounter)` | **翻訳（署名維持、時刻は Clock 経由）** | `resizer.rs` + 主型 impl |
| `System.nanoTime()` | **翻訳（Clock DI）** | `OptimalSizeExploringResizer<C: Clock>` |
| `ThreadLocalRandom` | **翻訳（seedable LCG）** | `lcg.rs` |
| `var performanceLog / record / stopExploring` | **翻訳（AShared、`stopExploring` は YAGNI 排除）** | `state.rs` |
| `checkTime = 0L` sentinel | **翻訳（`has_recorded: bool`）** | `resize_record.rs` |
| mailbox 観測 helper | **新設（Rust 流）** | `pool_router.rs` `observe_routee_mailbox_sizes` |
| kernel 側 `Resizer` trait 移設 | **非採用** | typed 単独で契約 1〜5 を実現可能 |
| `PoolSize = Int` | **非採用** | `usize` 直接使用 |
| `@SerialVersionUID` | **非採用** | JVM 固有 |
| `apply(Config)` HOCON | **非採用** | fraktor-rs に HOCON なし |
| `stopExploring` public setter | **非採用（YAGNI）** | Pekko でも `private[routing]` |
| `Routee::ActorRefRoutee` downcast | **非採用** | `observe_routee_mailbox_sizes` で `&[usize]` に抽象化 |

## 成果物

### プロダクションコード（typed）

- `modules/actor-core/src/core/typed/dsl/routing/optimal_size_exploring_resizer.rs`（新規、主型 + `Resizer` impl + `explore` / `optimize` helper + `libm_ceil` / `libm_floor`、約 350 行）
- `modules/actor-core/src/core/typed/dsl/routing/optimal_size_exploring_resizer/lcg.rs`（新規、seedable LCG、46 行）
- `modules/actor-core/src/core/typed/dsl/routing/optimal_size_exploring_resizer/resize_record.rs`（新規、`pub(crate) struct ResizeRecord<I>`、30 行）
- `modules/actor-core/src/core/typed/dsl/routing/optimal_size_exploring_resizer/state.rs`（新規、`pub(crate) struct State<I>`、21 行）
- `modules/actor-core/src/core/typed/dsl/routing/optimal_size_exploring_resizer/under_utilization_streak.rs`（新規、`pub(crate) struct UnderUtilizationStreak<I>`、14 行）
- `modules/actor-core/src/core/typed/dsl/routing/resizer.rs`（既存編集、`resize` 署名変更 + `report_message_count` default no-op 追加）
- `modules/actor-core/src/core/typed/dsl/routing/default_resizer.rs`（既存編集、`resize` 署名追従。挙動不変）
- `modules/actor-core/src/core/typed/dsl/routing/pool_router.rs`（既存編集、`observe_routee_mailbox_sizes` helper + `report_message_count` 呼び出し + `resize(&[usize])` 配線）
- `modules/actor-core/src/core/typed/dsl/routing.rs`（既存編集、`mod optimal_size_exploring_resizer;` + `pub use optimal_size_exploring_resizer::OptimalSizeExploringResizer;`）

### テスト

- `modules/actor-core/src/core/typed/dsl/routing/optimal_size_exploring_resizer/tests.rs`（write_tests ステップで新規、15 ケース + `FakeClock` / `FakeInstant` / `SEED` 定数）
- `modules/actor-core/src/core/typed/dsl/routing/pool_router/tests.rs`（write_tests ステップで smoke test 1 件追加）
- `modules/actor-core/src/core/typed/dsl/routing/default_resizer/tests.rs`（implement ステップで `resize` 新署名追従のみ、挙動不変）

### ドキュメント

- `docs/gap-analysis/actor-gap-analysis.md` を第 7 版に更新
  - `OptimalSizeExploringResizer`（core/kernel）を「実装済み（採用 + 翻訳）」に昇格（実装先は core/typed）
  - typed `OptimalSizeExploringResizer` expose（core/typed）を「実装済み」に昇格
  - サマリー: ギャップ 2 → 0、実装 99 → 101、カバレッジ 98% → 100%
  - Phase 3 hard セクションを全項目 closing に更新
- `docs/plan/pekko-porting-batch-4-optimal-size-exploring-resizer.md`（本ドキュメント）

## スコープ外

| 項目 | 理由 |
|------|------|
| kernel 側 `Resizer` trait 新設 | §「採用した判定結果」で非採用判定済み。「Pekko に似せることの目的化」に該当 |
| `apply(Config)` HOCON ファクトリ | 非採用判定済み。fraktor-rs には HOCON がない |
| `stopExploring` の公開 setter | YAGNI 判定。Pekko でも外部非公開 |
| receptionist facade / protocol / runtime 再配置（Phase 2 medium）| Batch 5 以降の構造整理フェーズで扱う |
| typed delivery `internal/` 層新設（Phase 2 medium） | 同上 |
| classic kernel `pub` 露出縮小（Phase 2 medium） | 同上 |

## 未来の判定変更トリガ

以下のいずれかが発生した場合は再判定を行うこと。

1. `stopExploring` による探索打ち切りを外部から制御する要件が発生 → public setter 追加を再検討
2. `Resizer` trait の責務を kernel に移設する別件要件が発生 → trait 配置を再検討
3. `SpinSyncMutex` 保護下の state access が性能ボトルネックとなる計測が出た → `AShared` パターンへの分離、もしくは lock-free データ構造への移行を再検討
4. Pekko `DefaultOptimalSizeExploringResizer` の契約 1〜5 や 10 チューニングパラメータのデフォルト値が変化した → 契約追随で実装方針を再検討

いずれも現時点では発生していない。
