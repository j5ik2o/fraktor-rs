# Research Log: Cellactor Actor Core 初期実装

## Decision 1: ActorSystem スコープ制御 API をライフタイム拘束付きコンテキストにする
- **Decision**: `ActorSystem::with_scope(|scope| { ... })` 形式で `ActorSystemScope<'sys>` を貸し出し、そこから生成される `ActorRef<'scope, M>` と `ActorContext<'scope, M>` は `'scope` ライフタイムを保持する。`ActorSystemScope` には `Drop` 実装で監査ログとリソース開放を行い、`Send`/`Sync` 境界を最小化した新設トレイト `ScopedActorRef` を導入する。  
- **Rationale**: protoactor-go の `RootContext` は `ActorSystem` インスタンスへ直接アクセスできるが、Rust ではライフタイムで外部流出を防ぐことで仕様の「スコープ外へムーブ禁止」をコンパイル時に保証できる。Apache Pekko Typed でも `ActorSystem` はガーディアン経由で参照され、スコープ外利用には安全装置があるため、類似の制約を Rust の所有権で表現するのが最も自然。  
  - 参照: `references/protoactor-go/actor/root_context.go` は `RootContext` を複製可能にしつつ ActorSystem を閉じ込めている。`references/pekko/actor-typed/.../ActorSystem.scala` では guardian 経由でメッセージ送信を制御。  
- **Alternatives considered**: 
  1. `ArcShared<ActorSystemInner>` を直接渡し `Send + Sync` を保持する案 → スコープ外ムーブを禁止できず命名規約 (`Handle` 禁止) に抵触。 
  2. `ScopedHandle` のような newtype を返しランタイム検査で無効化する案 → 仕様が求める「コンパイルまたは実行時検証」のうち、可能な限りコンパイル時に弾く方針と合致しない。

## Decision 2: メールボックス内部バッファに utils-core の SyncQueue/AsyncQueue を採用
- **Decision**: `modules/utils-core::v2::collections::queue::SyncQueue` / `AsyncQueue` を mailbox のデフォルトバックエンドに採用し、Props で `bounded::<capacity>()` や `priority()` を選択できる API を用意する。オーバーフローポリシーは `backend::OfferOutcome` と `QueueError` をラップして `MailboxOverflow` 列挙に変換する。  
- **Rationale**: utils-core の `SyncQueue` は `Shared` 抽象と `SpinSyncMutex` を使っており、命名規約と no_std 制約を自然に満たす。`offer/poll` が既に backpressure シグナル (`OfferOutcome::Full`) を返すため、FR-MBX-001/004 の実現が容易。`VecDeque` を独自に包むよりも、既存テスト (`modules/utils-core/src/v2/collections/queue/tests.rs`) が豊富な SyncQueue を利用した方が信頼性・整合性が高い。  
- **Alternatives considered**: 
  1. `alloc::collections::VecDeque` を直接利用しつつ `portable-atomic` でカウンタ管理する案 → `Shared` 命名規約とバックプレッシャーポリシーを自前で再発明する必要があり整合性が崩れる。 
  2. `heapless::spsc::Queue` を採用する案 → MPSC / 優先度 / 可変容量の要件を満たせず、組込み最適化に偏り過ぎる。

## Decision 3: EventStream 観測チャンネルは push ベースの Observer イベントを発火
- **Decision**: EventStream は内部キューとして SyncQueue を用い、購読者には `ObservationChannel<EventStreamMetric>` を push する。`EventStreamMetric` は publish/drop/subscribe/unsubscribe/backpressure の各イベントを列挙し、監視側は非同期ストリームまたは pull API で受け取れるようにする。Slow consumer には BackpressureHint を付与し、必要に応じてドロップ戦略を決定させる。  
- **Rationale**: protoactor-go の `eventstream.EventStream` は `Publish` 毎に同期でハンドラを呼び出し、購読解除時にアトミック操作で状態を更新する。これを踏襲しつつ、スペックでは監視イベントとメトリクス配信が必須のため push ベースで発火し、観測チャンネル経由で quickstart/テストが検証可能となる。pull ベースにすると監視側ポーリングでレイテンシが増え、SC-005 の遅延条件を満たしにくい。  
  - 参照: `references/protoactor-go/eventstream/eventstream.go` の Publish/Subscribe 実装。  
- **Alternatives considered**: 
  1. Metrics を `AtomicU64` カウンタとして expose し、利用者が pull する案 → backpressure のヒントを即時伝達できないため仕様 FR-EVT-004 と乖離。 
  2. `quickcheck` 的なテストでのみ内部チャンネルを利用し API には露出しない案 → 観測 API を利用した成功指標検証 (SC-005) を満たせない。

## 結論
上記 3 点の決定により、テクニカルコンテキストの `NEEDS CLARIFICATION` は解消された。以降のフェーズではこの方針に沿ってデータモデルと契約仕様を具体化する。
