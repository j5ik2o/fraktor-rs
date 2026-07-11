# スライド生成素材: Rustで宣言的ストリームDSLを設計する

このファイルは、外部ツール（Skywork）がスライドを生成するための自己完結型の素材集である。
GitHub リポジトリ（j5ik2o/fraktor-rs）へのアクセスは不要。コード抜粋はすべて実リポジトリからの正確な引用である。

---

## 0. 講演メタ情報

| 項目 | 内容 |
|------|------|
| タイトル | Rustで宣言的ストリームDSLを設計する: async boundary と island 実行モデル |
| イベント | 関数型まつり 2026（FP Matsuri）Track C |
| 日時 | 2026年7月12日 11:30〜（50分枠。本編45分 + Q&A 5分を想定） |
| スピーカー | かとじゅん（@j5ik2o） |
| セッションテーマ | 言語、実践手法、ライブラリ/フレームワーク |
| 想定聴衆 | Rust の基本と所有権概念、関数合成、Iterator/async/Stream の基礎理解がある人。バックプレッシャーやグラフベース処理の経験は不問 |

### 採択済みアブストラクト（この約束を全て回収する構成にする）

> Rust で宣言的なストリーム DSL を作るとき、難しいのは map や filter を並べる表面 API よりも、それをどう実行系に落とすかです。fraktor-rs を題材に、Source / Flow / Sink という API と Materializer / GraphInterpreter の設計を紹介。島（island）の仕組みにより、async boundary で分割されたグラフが複数の独立した実行単位として、バックプレッシャーと完了伝播を実現します。所有権、Send/Sync、no_std/std 分離との折り合い方について実践的な論点を共有するセッションです。

---

## 1. 題材プロダクト: fraktor-rs とは

このセクションの情報だけで「fraktor-rs とは」のスライドが作れるように記載する。ここに書かれていない機能・特徴を創作しないこと。

### 一言でいうと

**Apache Pekko と Proto.Actor に着想を得た、仕様駆動（specification-driven）の Rust アクターランタイム。** 移植性の高い `no_std` コアクレート群と、Tokio・ネットワーク等のホスト固有実装を担う `std` アダプタクレート群を分離して開発されており、**組込みとサーバーの双方で同じ契約（コア）を再利用できる**ことを最重視している。

### 基本情報

| 項目 | 内容 |
|------|------|
| リポジトリ | github.com/j5ik2o/fraktor-rs |
| 開発者 | かとじゅん（@j5ik2o、本トークのスピーカー） |
| ライセンス | MIT / Apache-2.0 デュアルライセンス |
| 状態 | pre-release（crates.io にパッケージ名は確保済み。API は `modules/` 配下のワークスペースクレート群が実体） |
| 動かし方 | `cargo run -p fraktor-showcases-std --example stream_first_example` 等の実行可能 showcase を多数同梱 |

### 提供している機能領域（stream は全体の一部）

- **Actor**: actor ref、supervision（監督）、death watch、routing、dispatcher、mailbox、event stream。untyped kernel の上に typed actor facade（typed API / DSL / receptionist / pub-sub）
- **Persistence**: event sourcing、journal、snapshot、persistent actor / FSM、durable state
- **Remote**: address / association / transport / wire format / failure detector
- **Cluster**: Virtual Actor（Grain）ランタイム基盤 — identity lookup、placement、activation/passivation、topology、downing
- **Stream**: 本トークの主題。宣言的 DSL、graph shape、stage / materialization 契約、queue、kill switch、supervision、actor 統合

### アーキテクチャ上の特徴（「fraktor-rs とは」スライドの中身）

1. **no_std / std 分離**: 全ドメインが `*-core`（`#![no_std]`。組込みでもサーバでも動く共通コア）と `*-adaptor-std`（Tokio 等の std 環境向けアダプタ）の2層構成
2. **依存方向（port-and-adapter）**: `*-core` が port（契約）を定義し、`*-adaptor-std` がそれを実装する。core は std を知らない
3. **参照実装からの逆輸入**: `references/pekko`（Scala）と `references/protoactor-go`（Go）をリポジトリ内に置き、設計をそのまま写経せず **Rust の型・所有権・no_std 制約に合わせて再設計**する開発スタイル
4. **仕様駆動 + 機械的ガード**: OpenSpec による仕様管理、参照実装とのギャップ分析、カスタム dylint（11本の自作 lint）と CI で設計意図とモジュール境界を機械的に守る

### 想定ユースケース

- Pekko / Akka 系アクターモデルを Rust で使いたい・検証したいランタイム開発
- `no_std` 制約下でも成立するアクター / ストリーム / 永続化の状態機械・契約設計
- ホスト固有実装（Tokio、TCP）をコアから切り離した port-and-adapter 型のランタイム実装

### stream 関連モジュール（本トークの範囲）

| モジュール | 役割 | 規模 |
|---|---|---|
| `stream-core-kernel` | DSL・実行系（Materializer / GraphInterpreter / island 分割）本体。`#![no_std]` | 実装 45,314行 / テスト 42,500行 |
| `stream-core-actor-typed` | typed actor 連携（PubSub の source/sink）。no_std | 実装 285行 / テスト 415行 |
| `stream-adaptor-std` | std 依存 I/O（FileIO / StreamConverters 等） | 実装 870行 / テスト 751行 |

### 実績数値（イントロおよびまとめで使用）

- 3つのstreamクレートにあるpublicな `struct` / `enum` / `trait` / `type` 宣言は合計236個
- `stream-core-kernel` の `*_test.rs` は42,500行。実装45,314行とほぼ1:1
- Pekkoとの固定スコープ比較は**47/50概念 = 94%カバー**。スコープ定義と内訳は `docs/gap-analysis/stream-gap-analysis.md` を参照
- 残ギャップは TCP/TLS の std アダプタ統合のみ
- 実装済みオペレータ例: map / filter / scan / stateful_map / conflate / expand / grouped / sliding / throttle / split_when / flat_map_merge / zip系 / merge系 / broadcast / balance / partition / hub系（broadcast_hub / merge_hub / partition_hub）/ kill_switch / restart系 / StreamRef（ActorSystem 間ストリーム参照）

計測日: 2026-07-10。再計測コマンド:

```bash
rg -g '*.rs' '^\s*pub (struct|enum|trait|type)\b' modules/stream-core-kernel modules/stream-core-actor-typed modules/stream-adaptor-std | wc -l
find modules/stream-core-kernel -type f -name '*_test.rs' -print0 | xargs -0 wc -l
find modules/stream-core-kernel/src -type f -name '*.rs' ! -name '*_test.rs' -print0 | xargs -0 wc -l
```

---

## 2. 貫通テーマとキーメッセージ

**貫通テーマ**: 宣言的ストリーム DSL の本体は「記述（blueprint）と解釈（materialization）の分離」である。表面 API は簡単、難しいのは実行系。

聴衆に持ち帰らせるメッセージ（重要度順）:

1. **「async boundary」は Pekko 用語の actor（スレッド）境界であって、Rust の async/await のことではない**。この語彙の衝突を解きほぐすこと自体が本トーク最大の持ち帰り
2. 並行性の単位は island。async boundary マーカーでグラフが island に分割され、island = 1 actor として独立駆動される
3. バックプレッシャーは demand（要求量）ベース。Sink 側が demand を出さない限り上流から値は流れない
4. wake 通知と tick ポーリングはどちらも no_std で実装できる。現在は wake 配線を避ける代わりに、最大 drive 間隔の完了検知遅延を受け入れている
5. 所有権を返す共有 API は、`FnOnce` で値を受け取り、任意の戻り値 `R` として外へ戻せる形にする

---

## 3. アウトライン（セクション別詳細）

### セクション1: イントロ「表面は簡単、実行が難しい」（約7分30秒 / 7枚）

- 自己紹介（1枚）
- **「fraktor-rs とは」を独立した2枚でしっかり説明する**:
  - 1枚目「何であるか」: Apache Pekko / Proto.Actor のセマンティクスを Rust にもたらすアクターランタイム。最大の特徴は **no_std（組込み）と std（Tokio）の両環境で同じコアが動く**こと。現在 pre-release で開発中
  - 2枚目「全体像と本トークの位置」: `{utils, actor, persistence, remote, cluster, stream}` の6ドメイン × core / adaptor-std の2層構成（図解E）。本トークはこのうち **stream 層**の話。参照実装（`references/pekko`, `references/protoactor-go`）を読みながら Rust イディオムに変換して逆輸入する開発スタイル。実績数値（3つのstreamクレートのpublic型宣言236、Pekko固定50概念中47概念で94%カバー、stream-core-kernelのテスト約4.2万行）をここで見せる
- 問いの提示: Actor があるのに、なぜ Stream が必要なのか？（1枚）
  - Actor = メッセージ配送・逐次処理・物理実行単位
  - Stream = 処理グラフ・需要量・終端伝播を宣言するデータフローモデル。物理実行では island = 1 actor として Actor 基盤を使う
- 本トークの地図: 「DSL（記述）→ Materializer（解釈）→ island（物理実行）」の3層を順に降りていく（図解A）（1枚）

### セクション2: 表面 — 宣言的 DSL の設計（約5分30秒 / 5枚）

- `Source<Out, Mat>` / `Flow<In, Out, Mat>` / `Sink<In, Mat>`: 要素型 + materialized value の**二段ジェネリクス**（コード抜粋 #3）
- 最小の実行例（コード抜粋 #1）: `Source::single(41).map(|v| v + 1).into_mat(Sink::head(), KeepRight)`
- 合成: `via` / `to` / `into_mat` と `KeepLeft` / `KeepRight` / `KeepBoth` — materialized value の合成規則を**型レベルで選択**させる `MatCombineRule` トレイト（コード抜粋 #5）。FP 聴衆に最も刺さるポイント
- `RunnableGraph` まで組んでも何も起きない。`run(&mut materializer)` で初めて実行される = **不変 blueprint**
- 小ネタ: Pekko の `.async` は Rust では予約語なので `r#async()`（生識別子）で実装（コード抜粋 #4）。ここで「async boundary」という語を初出させ、「ただの属性マーカー」とだけ予告して詳細はセクション4へ先送り

### セクション3: 解釈 — Materializer と GraphInterpreter（約7分45秒 / 6枚）

- `ActorMaterializer::materialize()` の5ステップ:
  1. `RunnableGraph` から `StreamPlan`（トポロジカルソート済みの不変プラン）を取り出す
  2. `IslandSplitter::split(plan)` で async boundary を境に island に分割
  3. island 間の cross edge に `IslandBoundaryShared`（有限FIFO）を挿入し、境界用の合成 Sink/Source ステージを追加
  4. 各 island を `GraphInterpreter` にコンパイル
  5. island ごとに `StreamIslandActor` を actor system に spawn し、スケジューラが固定間隔（既定10ms）で `Drive` コマンドを送る（actor の仕組み自体は次セクション冒頭でおさらいする、と一言添える）
- **型消去の壁**: DSL 表面はジェネリクスだが、実行系内部は `DynValue = Box<dyn Any + Send + 'static>` が流れる。型不一致は実行時に `StreamError::TypeMismatch`。ジェネリック DSL と動的グラフ実行のブリッジをどこで切るかという Rust 固有の設計判断
- `GraphInterpreter` = 単一スレッド・協調的ステートマシン。`drive()` 1回で「flow を tick → sink 起動 → pull と駆動 → sink を1回駆動 → source 完了確認」の1ステップが進む
- **バックプレッシャー（demand ベース）**: `DemandTracker` が `Demand::{Finite(u64), Unbounded}` を管理（コード抜粋 #8）。Sink の demand がない限り upstream から pull しない。同一 island 内はステージの `can_accept_input()` + 固定容量エッジバッファで流量制御

### セクション4: 核心 — async boundary と island 実行モデル（約13分20秒 / 10枚）※最重量セクション

- **冒頭に「アクター90秒おさらい」（1〜2枚）**: アクターモデルの経験がない聴衆向けに、このトークで使う3点だけを説明する。フルの入門（監督戦略・階層・位置透過性・typed/untyped）には踏み込まない
  1. actor = 状態 + mailbox。メッセージを**1つずつ逐次処理**する（だから actor 内はロック不要で `&mut` のまま状態を書き換えられる）
  2. メッセージ送信は fire-and-forget（後述の `Drive` / `Cancel` コマンドがこれ）
  3. dispatcher が actor をスレッドに割り当てる（`async_with_dispatcher` の意味がここで繋がる）
- **キーメッセージの提示**: 「async boundary」は Rust の async/await ではない。Pekko 用語の「actor 境界」= 並行実行単位の境界
- `AsyncBoundaryAttr` はただのマーカー属性（unit struct）。`.r#async()` は最後のステージに属性を付けるだけ
- `IslandSplitter` の分割アルゴリズム（コード抜粋 #7 + 図解B）:
  - エッジを走査し、上流ステージに async 属性があればそのエッジは「切断点」として隣接関係を作らない
  - 残りのエッジで無向グラフの連結成分（BFS）を計算 → 連結成分 = island
  - dispatcher 属性は下流 island に伝播し、その island の actor が指定 dispatcher で spawn される
- island 間の接続: `IslandBoundary` = 容量16（既定）の bounded FIFO（コード抜粋 #6 + 図解C）
  - push は上流 island の BoundarySink、pull は下流 island の BoundarySource が担当
  - 状態機械: `Open → Completed / Failed(error) / DownstreamCancelled`
- **完了・エラー伝播の順序保証**: バッファに残った要素を flush し切るまで、完了/エラーの終端シグナルを意図的に遅延させる（pending terminal）。「データより先に『終わった』が届く」事故を防ぐ
- **キャンセル伝播**: 下流 island の cancel は制御プレーン（DownstreamCancellationControlPlane）経由で上流 island の actor に `Cancel` コマンドとして配送。配送失敗時は kill switch で全 island を fail-fast
- island の物理実行: `StreamIslandActor` が `StreamIslandCommand::{Drive, Cancel, Shutdown, Abort}` を mailbox で受けて `GraphInterpreter::drive()` を進める。ストリームが終端状態になったら actor は自分を stop
- **正直なトレードオフの開示（本トークのハイライト）**: `map_async` の Future は `noop_waker`（何もしない Waker）で tick ごとにポーリングされる（コード抜粋 #9）。wake 通知と tick ポーリングはどちらも no_std で実装でき、現在は wake 配線を避ける代わりに最悪 drive 間隔（既定10ms）の完了検知遅延を受け入れている
  - 「async/await の皮を被った協調・固定間隔ポーリング」という設計判断。wake 配線の単純さ vs レイテンシ

### セクション5: Rust との折り合い — 所有権・Send/Sync・no_std（約4分50秒 / 4枚）

- プロジェクト全体の内部可変性ポリシー: ロジックは `&mut self` で設計し、共有が必要な箇所だけ `SharedLock`（クロージャ型 API `with_read`/`with_write`、ガードを外に返さない）でラップ
- **所有権が API を規定する実例**: `SharedAccess::with_write` は `FnOnce` で値をクロージャへ move し、任意の戻り値 `R` として `Err(value)` を外へ戻せる。`IslandBoundaryShared` の直接ロックは所有権上の必然ではなく、`SharedLock` へ寄せられるリファクタリング候補
- ステージ間を流れる値は `Box<dyn Any + Send>`: Send 境界が全ステージの要素型に要求される
- **層分離**: Tokio との結線点は Stream 層ではなく Actor 層（dispatcher / tick driver）。`stream-core-kernel` は `#![no_std]` のまま、std/embedded の差し替えは actor 層で吸収。`ActorSystemConfig::new(StdTickDriver::default())` → `ActorSystem` → `ActorMaterializer::new(system, config)` という接続経路（コード抜粋 #1 の main 関数がそのまま例になる）

### セクション6: 残課題・まとめ・ありがとう（約2分20秒 / 3枚）

- 残課題を率直に: TCP/TLS の std 統合、GraphInterpreter drive loop の分割（demand/scheduling を壊さない単位で段階的に）
- 持ち帰り3点:
  1. 宣言的 DSL の本体は「記述と解釈の分離」— blueprint は不変データ、実行は interpreter
  2. async boundary ≠ async/await。並行性の単位は island（= actor）
  3. no_std を貫くなら実行モデルごと自作する覚悟が要る — その対価と見返り
- ありがとうスライド: github.com/j5ik2o/fraktor-rs と @j5ik2o を再掲して締める

### スコープ外（言及のみ、深入りしない）

GraphDsl（fan-in/out ビルダー）、StreamRef、hub 系、restart 系、throttle の詳細は「今日は触れないが存在する」と1枚で言及するに留める。50分に詰め込むと全部が浅くなるため。

---

## 4. コード抜粋集（実リポジトリからの正確な引用）

### 抜粋 #1: 最小の実行例（showcases/std/stream/first-example/main.rs 全文）

```rust
use std::{error::Error, time::Duration};

use fraktor_actor_adaptor_std_rs::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::{actor::setup::ActorSystemConfig, system::ActorSystem};
use fraktor_stream_core_kernel_rs::{
  dsl::{Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, KeepRight},
};

fn main() -> Result<(), Box<dyn Error>> {
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::create_with_noop_guardian(config)?;
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start()?;
  // 失敗時にも materializer.shutdown() を必ず通すため、実行本体をクロージャに閉じる
  let outcome = {
    let mut run = || -> Result<(), Box<dyn Error>> {
      let graph = Source::single(41_u32).map(|value| value + 1).into_mat(Sink::head(), KeepRight);
      let running = graph.run(&mut materializer)?;
      let result = running.materialized().wait_blocking(&StdBlocker::new())?;
      assert_eq!(result, 42);
      println!("stream_first_example result: {result}");
      Ok(())
    };
    run()
  };
  let shutdown_result = materializer.shutdown();
  // 実行エラーを優先して報告する。両方失敗した場合、shutdown 側のエラーは実行失敗の帰結のため省く
  outcome?;
  shutdown_result?;
  Ok(())
}
```

### 抜粋 #2: 合成の例（showcases/std/stream/composition/main.rs より）

```rust
let graph = Source::from_array([1_u32, 2])
  .via(Flow::new().concat_lazy(Source::from_array([3_u32, 4])))
  .into_mat(Sink::collect(), KeepRight);
let materialized = graph.run(&mut materializer)?;
let values = materialized.materialized().wait_blocking(&StdBlocker::new())?;
assert_eq!(values, vec![1, 2, 3, 4]);
```

### 抜粋 #3: Source の型定義（dsl/source.rs）— 二段ジェネリクスと不変 blueprint

```rust
/// Source stage definition.
pub struct Source<Out, Mat> {
  graph: StreamGraph,          // 不変 blueprint（ステージ列 + エッジ + 属性）
  mat:   Mat,                  // materialized value（実行時に得られる値の型）
  _pd:   PhantomData<fn() -> Out>,
}
```

### 抜粋 #4: async boundary は属性マーカー（dsl/flow.rs）

```rust
/// Marks this flow with an async boundary attribute.
///
/// The materializer uses this attribute to split the graph into
/// independently executed islands. The boundary is resolved at
/// materialization time and does not insert a buffer stage.
///
/// Mirrors Pekko's `Graph.async`.
#[must_use]
pub fn r#async(mut self) -> Flow<In, Out, Mat> {   // `async` は予約語なので生識別子
  self.graph.mark_last_node_async();
  self
}

/// Marks this flow with an async boundary attribute and a named dispatcher.
#[must_use]
pub fn async_with_dispatcher(mut self, dispatcher: impl Into<String>) -> Flow<In, Out, Mat> {
  self.graph.mark_last_node_async();
  self.graph.mark_last_node_dispatcher(dispatcher);
  self
}
```

属性の実体はただの unit struct:

```rust
// attributes/async_boundary_attr.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncBoundaryAttr;
```

### 抜粋 #5: materialized value の合成規則を型で選ぶ（materialization/mat_combine_rule.rs）

```rust
/// Type-level rule for combining materialized values.
pub trait MatCombineRule<Left, Right> {
  /// Output type produced by the combination.
  type Out;

  /// Returns the combination kind.
  fn kind() -> MatCombine;

  /// Combines materialized values according to the rule.
  fn combine(left: Left, right: Right) -> Self::Out;
}

// KeepLeft / KeepRight / KeepBoth / KeepNone が実装として提供される
pub struct KeepRight;
```

### 抜粋 #6: island 間の bounded FIFO（impl/interpreter/island_boundary.rs）

```rust
/// Default capacity for inter-island boundary buffers.
pub(crate) const DEFAULT_BOUNDARY_CAPACITY: usize = 16;

/// Lifecycle state of an `IslandBoundary`.
pub(crate) enum BoundaryState {
  /// The boundary is open for push/pull.
  Open,
  /// The upstream has completed normally. Remaining buffered elements
  /// should be drained before the downstream observes completion.
  Completed,
  /// The upstream has failed. Remaining buffered elements should be
  /// drained before the downstream observes the error.
  Failed(StreamError),
  /// The downstream island has cancelled demand across this boundary.
  DownstreamCancelled,
}

/// Bounded FIFO buffer between two islands.
pub(crate) struct IslandBoundary {
  buffer:           VecDeque<DynValue>,
  capacity:         usize,
  pub(crate) state: BoundaryState,
}

/// Attempts to push a value into the buffer.
///
/// Returns `Ok(())` on success. Returns `Err(value)` when the buffer
/// is full or the boundary is no longer open, giving the value back to
/// the caller.
pub(crate) fn try_push(&mut self, value: DynValue) -> Result<(), DynValue> { /* ... */ }
```

所有権が API を規定する実例（rustdoc 原文）:

```rust
/// Shared, clone-able handle to an `IslandBoundary`.
///
/// Uses `ArcShared<SpinSyncMutex<IslandBoundary>>` for lock-based access
/// because `try_push` returns ownership of the rejected value, which
/// cannot be expressed through a `SharedAccess`-style closure API.
#[derive(Clone)]
pub(crate) struct IslandBoundaryShared {
  inner: ArcShared<SpinSyncMutex<IslandBoundary>>,
}
```

上記は現行実装の rustdoc 原文だが、技術的な必要条件ではない。`SharedAccess::with_write` は `FnOnce` と任意の戻り値 `R` を持つため、次の形で拒否された値の所有権を外へ戻せる。

```rust
let result = boundary.with_write(move |inner| inner.try_push(value));
```

スライドでは直接ロックを正当化せず、`FnOnce + R` が所有権を保った共有 API を表現できる点を扱う。

### 抜粋 #7: island 分割アルゴリズム（impl/interpreter/island_splitter.rs、要点）

```rust
/// Splits a `StreamPlan` into islands at async boundary markers.
///
/// Semantics: a stage with `is_async()` attribute is the **last** stage
/// in its current island. The next stage in topological order starts a
/// new island.
pub(crate) struct IslandSplitter;

// assign_islands の核心部: async 属性付きステージから出るエッジは
// 「切断点」として隣接関係に加えない。残りのエッジで連結成分（BFS）を
// 計算し、連結成分 = island とする。dispatcher 属性は下流 island に伝播。
for edge in &plan.edges {
  // ...（from_stage / to_stage の解決）...
  if plan.stages[from_stage].attributes().is_async() {
    if let Some(dispatcher) = plan.stages[from_stage].attributes().get::<DispatcherAttribute>() {
      dispatcher_candidates[to_stage].push(String::from(dispatcher.name()));
    }
    continue; // このエッジでは island を繋がない = 切断
  }
  adjacency[from_stage].push(to_stage);
  adjacency[to_stage].push(from_stage);
}
// この後 BFS で連結成分を計算し、トポロジカル順に island ID を採番
```

### 抜粋 #8: demand ベースのバックプレッシャー（impl/fusing/demand_tracker.rs）

```rust
/// Tracks aggregated demand and handles saturation.
pub struct DemandTracker {
  demand: Demand,   // Demand::Finite(u64) または Demand::Unbounded
}

impl DemandTracker {
  /// Adds demand to the tracker.
  pub const fn request(&mut self, amount: u64) -> Result<(), StreamError> {
    if amount == 0 {
      return Err(StreamError::InvalidDemand { requested: amount });
    }
    match self.demand {
      | Demand::Unbounded => Ok(()),
      | Demand::Finite(current) => {
        let next = current.saturating_add(amount);
        if next == u64::MAX {
          self.demand = Demand::Unbounded;
        } else {
          self.demand = Demand::Finite(next);
        }
        Ok(())
      },
    }
  }
  // consume(amount) で demand を減算。残量超過は StreamError::DemandExceeded
}
```

Sink 側の全コールバック（`on_start` / `on_push` / `on_tick`）が `&mut DemandTracker` を受け取り、Sink がいつ・どれだけ demand を出すかを完全に制御する。interpreter は `has_demand()` が真のときだけ上流から pull する。

### 抜粋 #9: noop_waker による tick ポーリング（impl/fusing.rs + map_async_logic.rs）

```rust
// impl/fusing.rs — 何もしない Waker（no_std で構築可能）
pub(crate) const fn noop_waker() -> Waker {
  unsafe { Waker::from_raw(noop_raw_waker()) }
}

const fn noop_wake(_: *const ()) {}   // wake されても何も起きない
```

```rust
// impl/fusing/map_async_logic.rs — Future は tick のたびにポーリングされる
fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
  let waker = noop_waker();
  let mut cx = Context::from_waker(&waker);
  for entry in &mut self.pending {
    let MapAsyncEntry::InFlight(future) = entry else { continue };
    if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
      *entry = MapAsyncEntry::Completed(output);
    }
  }
  // 完了した Future の結果を FIFO 順で放出（先頭が未完了なら順序を守って停止）
  // ...
}
```

意味: 実行エンジンは wake 通知と統合されておらず、Future の完了検知は次の drive tick（既定10ms、設定可能）まで遅れる。wake 通知と tick ポーリングはどちらも no_std で実装可能であり、現在の選択は「wake 配線を要求しない単純さ」と「完了検知レイテンシ」の意図的なトレードオフである。

---

## 5. 図解指示（作図が必要なスライド）

### 図解A: 3層アーキテクチャ図（セクション1と3で使用）

縦3層の図:
1. 最上層「DSL（記述）」: `Source → Flow → Sink` のチェーンと `RunnableGraph<Mat>`。ラベル「不変 blueprint。まだ何も実行されない」
2. 中間層「Materializer（解釈）」: `StreamPlan` → `IslandSplitter` → `GraphInterpreter` × N。ラベル「blueprint を実行可能な island 群にコンパイル」
3. 最下層「Actor System（物理実行）」: `StreamIslandActor` × N と scheduler からの `Drive` tick（10ms間隔の時計アイコン）。ラベル「island = 1 actor。tick 駆動で前進」

### 図解B: island 分割の前後比較（セクション4で使用）

- 左（分割前）: `Source → map → filter →[async boundary]→ map_async → Sink` の直線グラフ。async boundary 位置に赤い破線
- 右（分割後）: Island 1（`Source → map → filter → BoundarySink`）と Island 2（`BoundarySource → map_async → Sink`）の2つの箱。間に「IslandBoundary（容量16の FIFO）」の箱。各 island の箱に「= 1 actor」のラベル

### 図解C: island 境界の状態と伝播（セクション4で使用）

中央に FIFO バッファ（容量16）。左から BoundarySink が push、右へ BoundarySource が pull。
- 下に状態機械: `Open → Completed`（正常完了）、`Open → Failed(e)`（エラー）、`Open → DownstreamCancelled`（下流キャンセル）
- 注記1: 「バッファ full の間、上流は pending に保持して次 tick で再送」
- 注記2: 「完了/エラーは、バッファの要素を流し切ってから下流に観測される（順序保証）」
- 注記3: 「下流キャンセルは制御プレーン経由で上流 island の actor に Cancel コマンドとして配送。配送失敗時は kill switch で全 island を fail-fast」

### 図解D（任意）: demand の流れ（セクション3で使用）

`Source ← Flow ← Sink` の逆向き矢印で「demand（要求量）」、順方向の矢印で「要素」。「Sink が request(n) しない限り要素は流れない」のラベル。

### 図解E: fraktor-rs ワークスペース構成図（セクション1「fraktor-rs とは」2枚目で使用）

6ドメイン × 2層のマトリクス図:
- 横軸: `utils` / `actor` / `persistence` / `remote` / `cluster` / `stream` の6ドメイン
- 縦軸（上下2層）: 上段 `*-core`（`#![no_std]`。組込みでもサーバでも動く共通コア）、下段 `*-adaptor-std`（Tokio 等の std 環境向けアダプタ）
- `stream` 列全体をハイライトし「本トークの範囲」ラベル
- 依存方向の注記: 「core が port（契約）を定義し、adaptor-std がそれを実装する。std が core を駆動するのではない」
- 脇に `references/pekko`（Scala）と `references/protoactor-go`（Go）の箱を置き、「参照実装。設計を Rust イディオムへ逆輸入」の矢印

---

## 6. 用語集（スライド内の表記を統一すること）

| 用語 | 表記ルール |
|------|-----------|
| Source / Flow / Sink | 英語のまま。訳さない |
| materialized value | 英語のまま（初出時に「実行時に得られる値」と補足可） |
| Materializer / GraphInterpreter | 型名なので英語のまま |
| blueprint | 「blueprint（設計図）」と初出時のみ併記 |
| async boundary | 英語のまま。**「非同期境界」と訳さない**（async/await との混同を招くため） |
| island | 「island（島）」と初出時のみ併記。以後 island |
| backpressure | 「バックプレッシャー」のカタカナ可 |
| demand | 英語のまま（「要求量」と初出時に補足可） |
| tick / drive | 英語のまま |
| actor / dispatcher / mailbox | 英語のまま |
| no_std / std | コード表記のまま |

---

## 7. トーン・スタイル指示

- 語り口: 実装者本人による一次情報の共有。「作ってみて分かったこと」を率直に。マーケティング調・誇張は禁止
- 弱点（tick ポーリングのレイテンシ、GraphInterpreter の巨大さ）は隠さず「意図的なトレードオフ」として堂々と提示する。これがこのトークの信頼性の源泉
- Pekko との関係: 「表面 API は Pekko 準拠、実行系は Rust/no_std の制約に合わせて再設計」という対比を一貫させる
- コードスライドは1枚1論点。抜粋をさらに削ってよいが、**改変・創作はしないこと**（このファイルにないコードをでっち上げない）
- 各セクションの冒頭に「いまここ」を示すアジェンダ再掲スライドを入れる
