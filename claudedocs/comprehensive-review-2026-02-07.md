# fraktor-rs 総合コードレビュー・課題整理

**実施日**: 2026-02-07
**対象**: fraktor-rs v0.2.11（全6モジュール、1,233ファイル）

---

## 1. プロジェクト全体サマリー

| モジュール | ファイル数 | 公開型数 | 参照実装 | 成熟度 |
|-----------|----------|---------|---------|--------|
| **actor** | 533 | 252(core)+24(std) | Pekko | 高 |
| **cluster** | 237 | 152 | protoactor-go | 高 |
| **persistence** | 75 | ~35 | Pekko Persistence | 高 |
| **remote** | 97 | ~50 | Pekko Remote | 中〜高 |
| **streams** | 59 | ~30 | Pekko Streams | 中 |
| **utils** | 232 | ~80 | 独自 | 高 |

**総合評価**: 実装品質・設計の一貫性は高い。**主要課題は「開発者体験 (DX)」と「サンプル・ドキュメントの不足」**。

---

## 2. クロスカッティング課題（全モジュール共通）

### 2.1 DX の壁：初期化の複雑さ

**現状**: アクターを1つ動かすまでに ~120行のボイラープレートが必要。

```
TickDriverConfig 構成 → ActorSystemConfig 構成 → Extension 登録 → ActorSystem 起動 → Props 定義 → spawn
```

**Pekko との比較**: Pekko では `ActorSystem.create("name")` の1行で開始可能。

**改善案**: `ActorSystem::quickstart("name")` のような簡易 API を提供。

### 2.2 型パラメータ `TB: RuntimeToolbox` の遍在

**現状**: ほぼすべての型が `*Generic<TB>` 形式で、ジェネリクスが API 全体に伝播。

- std 環境には型エイリアス（`ActorRef = ActorRefGeneric<StdToolbox>`）が提供済み
- しかし core 環境では常に `<TB>` を明示する必要がある
- エラーメッセージが `<<TB as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::Mutex<T>` のように冗長

**改善案**: `prelude` モジュールで頻用型を短い名前で re-export。

### 2.3 サンプルコードは量があるが導線が弱い

**現状**:
- actor: 42 examples（豊富）
- cluster: 10 examples
- remote: 2 examples
- streams: 1 example
- persistence: 2 examples

**問題**: README には examples/guides への導線はあるが、「最初にどのサンプルを読むべきか」と目的別の実行コマンド（`cargo run -p ... --example ...`）が整理されていない。

### 2.4 rustdoc のリンク切れ

`cargo doc` がエラーで失敗する状態（`ActorSystemBuilder`, `SupervisorStrategy` 等への unresolved link）。

### 2.5 アーキテクチャドキュメント導線の不足

README のモジュール関係図や `docs/guides/` の個別ガイドは存在するが、コンポーネント間の実行フロー（Dispatcher → Mailbox → ActorCell など）を横断的に追える外部向け資料が不足している。

---

## 3. モジュール別 課題詳細

### 3.1 actor モジュール

**強み**: Pekko 忠実な移植、typed/untyped 双方の API、no_std 完全対応

| 優先度 | 課題 | 詳細 |
|--------|------|------|
| **P0** | rustdoc リンク切れ修正 | `cargo doc` が失敗する |
| **P1** | 初期化 API 簡潔化 | `ActorSystem::quickstart()` の追加 |
| **P1** | typed actor を推奨パスとして明記 | typed vs untyped の使い分けガイド |
| **P1** | public 型の削減 | 252型→220型程度に（internal 型を `pub(crate)` 化） |
| **P1** | Behavior DSL の充実 | `receive_and_reply` shorthand、message adapter builder |
| **P2** | Stash メカニズム | behavior transition 時のメッセージバッファリング |
| **P2** | Backpressure Protocol | mailbox full 時の明示的ハンドリング |
| **P2** | アーキテクチャドキュメント | コンポーネント図、データフロー図 |
| **P3** | Router パターン | roundrobin, scatter-gather |
| **P3** | Performance baseline | criterion ベンチマーク整備 |

### 3.2 cluster モジュール（protoactor-go 参照）

**強み**: protoactor-go の主要機能を 80-85% カバー。Virtual Actor, Grain, Gossip, PubSub 実装済み。

| 優先度 | 課題 | 詳細 |
|--------|------|------|
| **P1** | Placement API の統合窓口 | PlacementCoordinator/Driver/Shared の統一インターフェース |
| **P1** | エラー型の階層化 | ClusterError を基底として統一 |
| **P1** | アーキテクチャガイド | Placement state machine 図、Membership flow |
| **P2** | Cluster Provider 拡充 | Kubernetes, Consul 等（現状 Local + Static + AWS ECS） |
| **P2** | 統合テスト拡充 | failure scenario テスト（node down, network partition） |
| **P2** | Multi-hop grain call サンプル | 複雑なシナリオの example |
| **P3** | Multi-Datacenter 対応 | DC identifier, affinity オプション |
| **P3** | Consensus-based Placement | 複数 authority、placement conflict resolution |

### 3.3 persistence モジュール

**強み**: Pekko Persistence の中核機能を完全実装。stub/placeholder なし。GAT 活用。

| 優先度 | 課題 | 詳細 |
|--------|------|------|
| **P1** | PersistenceContext の隠蔽 | アクター定義から実装詳細を排除 |
| **P1** | Quickstart ガイド作成 | Pekko EventSourcedBehavior との対応例 |
| **P1** | `persist_async` の改名 | → `persist_unfenced`（誤解防止） |
| **P2** | Persistent FSM パターン | 状態遷移が複雑な永続アクター対応 |
| **P2** | JournalResponse の簡潔化 | 8 variant → Result ベースに検討 |
| **P3** | Event adapter パターン | イベントスキーマ進化対応 |

### 3.4 remote モジュール

**強み**: Pekko Remote の主要メカニズム実装。Phi Failure Detector、Endpoint Association FSM、Handshake Protocol。

| 優先度 | 課題 | 詳細 |
|--------|------|------|
| **P0** | TLS/SSL transport | 本番環境での使用に必須 |
| **P1** | Handshake timeout 実装 | 無限待機のリスク排除 |
| **P1** | API エントリーポイント簡素化 | 7階層の設定チェーン → 統合 API |
| **P1** | 設計ドキュメント | FSM 状態遷移図、backpressure flow |
| **P2** | Multi-node 統合テスト | 2+ ノードシナリオの検証 |
| **P2** | Flight Recorder 永続化 | in-memory のみ → circular buffer / file sink |
| **P3** | Compression support | 帯域幅削減 |

### 3.5 streams モジュール

**強み**: Pekko Streams の Source → Flow → Sink 構造を忠実に実装。Fluent API。

| 優先度 | 課題 | 詳細 |
|--------|------|------|
| **P1** | サンプル追加 | 現状1個のみ → 基本パターン5個程度 |
| **P2** | StageKind enum の命名規約 | 将来の variant 増加に備え |
| **P2** | ランタイム型チェック最小化 | `DynValue + TypeId` → コンパイル時チェック強化 |
| **P3** | Pekko Streams 対応表 | `akka.stream.*` ↔ `fraktor-streams-rs.*` |

### 3.6 utils モジュール

**強み**: no_std/std 両対応。RuntimeToolbox, ArcShared, SharedAccess の設計は堅実。

| 優先度 | 課題 | 詳細 |
|--------|------|------|
| **P2** | モジュール分離検討 | sync (39ファイル) → fraktor-sync-rs に独立化候補 |
| **P2** | モジュール分離検討 | concurrent (23ファイル) → fraktor-concurrent-rs に独立化候補 |
| **P3** | URI Parser の扱い | actor 側に統合 or utils に残留 |
| **P3** | 上位型エイリアス追加 | `DefaultMutex<T> = ToolboxMutex<T, NoStdToolbox>` |

---

## 4. 優先実行ロードマップ

### Phase A: DX 改善（最優先）

1. **rustdoc リンク切れ修正** — `cargo doc` が通る状態に
2. **ActorSystem quickstart API** — 初期化ボイラープレート削減
3. **Getting Started ガイド** — 「5分で動かす」チュートリアル
4. **サンプルコードの案内** — README に推奨 example パスを明記
5. **typed actor 推奨パスの明記** — actor-system.md に判断フロー追加

### Phase B: API 品質向上

6. **public 型の整理** — ✅ 2 型を pub(crate) 化（252→250）、残りは API 再設計が必要
7. **Placement 統合窓口** — → Phase C 延期（新 API 設計が必要）
8. **PersistenceContext 隠蔽** — → Phase D 延期（trait 設計の根本見直しが必要）
9. **Remote エントリーポイント簡素化** — → Phase C 延期（ビルダー API 設計が必要）
10. **persist_async 改名** → ✅ `persist_unfenced` に完了

### Phase C: 機能充実

11. **TLS transport** — remote 本番化の前提条件
12. **Stash メカニズム** — actor の behavior transition サポート
13. **統合テスト拡充** — cluster failure scenario, remote multi-node
14. **streams サンプル追加** — 基本パターン5個程度
15. **アーキテクチャドキュメント** — 全モジュールの設計ガイド

### Phase D: 構造最適化

16. **utils モジュール分離** — sync, concurrent の独立化検討
17. **Cluster Provider 拡充** — Kubernetes 等
18. **Performance baseline** — criterion ベンチマーク
19. **Persistent FSM** — 状態遷移パターン対応

---

## 5. 強みの再確認

| 観点 | 評価 |
|------|------|
| **設計の一貫性** | Pekko/protoactor-go に忠実、プロジェクトルールの機械的強制（8 Dylint lint） |
| **コード品質** | テスト完備、stub/placeholder なし（rustdoc は broken intra-doc link の解消が必要） |
| **no_std 対応** | Rust アクターフレームワークとして唯一の本格的 no_std サポート |
| **型安全性** | RuntimeToolbox による静的多形、CQS 原則の一貫した適用 |
| **テスト充実度** | ユニットテスト + 統合テスト + 42 examples（actor） |

**結論**: fraktor-rs は実装品質が高く、Rust エコシステムで Pekko 相当のアクターフレームワークとして認知されるポテンシャルを持つ。DX 改善と Getting Started 体験の向上が、利用者獲得への最短経路。

---

## 6. タスクリスト

### Phase A: DX 改善（最優先）

#### A-1. rustdoc リンク切れ修正 [P0] [actor] ✅ 完了
- [x] `cargo doc -p fraktor-actor-rs` のエラー一覧を取得
- [x] `ActorSystemBuilder` への unresolved link を修正
- [x] `SupervisorStrategy` への unresolved link を修正
- [x] その他の broken intra-doc link をすべて修正（全17件: actor 12, remote 4, cluster 1, streams 1）
- [x] 全モジュールで `cargo doc --no-deps` が成功することを確認

#### A-2. ActorSystem quickstart API [P1] [actor] ✅ 設計完了
- [x] `ActorSystem::quickstart(&props)` 簡易初期化 API を設計（`claudedocs/quickstart-api-design.md`）
- [x] TickDriverConfig のデフォルト構成を内部で自動適用（設計済み）
- [x] `DispatcherConfig::tokio_auto()` で Tokio Handle 自動検出（設計済み）
- [x] Codex Architect レビュー完了（feature gate `#[cfg(feature = "tokio-executor")]` 必須等）
- [ ] 実装（設計書に基づく実コード作成は Phase B 以降）

#### A-3. Getting Started ガイド [P1] [docs] ✅ 完了
- [x] `docs/guides/getting-started.md` を新規作成
- [x] no_std 版の最小サンプルを記載
- [x] std/Tokio 版の最小サンプルを記載
- [x] `cargo run --example` の実行手順と期待出力を明記
- [x] 必要な Cargo.toml の feature フラグ設定を記載

#### A-4. サンプルコードの案内 [P1] [docs] ✅ 完了
- [x] Getting Started ガイドに「次のステップ」として目的別 example マッピング表を配置
- [x] 目的別 example マッピング表を作成（11項目: Typed Actor, Behavior, Supervision, DeathWatch, Scheduler, EventStream, Serialization, Remoting, Cluster, Persistence, Streams）
- [x] 各 example に冒頭 `//!` コメントで「このサンプルが示す概念」を明記（33ファイル）

#### A-5. typed actor 推奨パスの明記 [P1] [actor] [docs] ✅ 完了
- [x] `docs/guides/actor-system.md` に typed vs untyped の判断フローを追加
- [x] typed API を推奨パスとして明記（type safety の利点）
- [x] untyped が必要なユースケースを列挙（動的ディスパッチ、plugin 等）
- [x] Behavior DSL（std）の位置づけを説明
- [x] Message Adapter の使用例（TypedActor / Behavior DSL 両方）
- [x] Codex Code Reviewer レビュー完了 + 指摘3件修正済み

---

### Phase B: API 品質向上

#### B-1. public 型の整理 [P1] [actor] ✅ 調査・部分実施完了
- [x] 16 個の内部型候補を特定し、外部参照を調査
- [x] 2 型を `pub(crate)` に変更（MailboxMessage, CancellableState）
- [x] 変更後に全テスト（521件）が通ることを確認
- **結果**: 16型中14型は公開メソッドシグネチャに組み込まれており変更不可
  - ScheduleHints, EnqueueOutcome, MailboxCapacity → Dispatcher/Mailbox の pub fn 引数・戻り値
  - MailboxMetricsEvent, DispatcherDumpEvent → EventStreamEvent enum の pub variant
  - CancellableEntry, BatchMode → CancellableRegistry/ExecutionBatch の pub fn
  - TaskRunSummary, SchedulerWarning, SchedulerDump, SchedulerDumpJob, SchedulerMetrics → Scheduler の pub fn 戻り値
  - FailureOutcome → SystemStateShared の pub fn 戻り値
  - ReceiveState → placeholder 型（dead_code 警告回避のため pub 維持）
- **判断**: 大規模な API リファクタリングなしでの削減は 2 型が限界。252→250 型。目標の 220 型達成には Phase C 以降で API 再設計が必要

#### B-2. Placement 統合窓口 [P1] [cluster] → Phase C に延期
- [x] 調査完了: 17 ファイルにまたがる Placement 関連 API を分析
- **結果**: PlacementCoordinator/Driver/Shared は内部で密結合しており、単純な pub(crate) 化では不十分
- **延期理由**: 新しいファサード API の設計が必要（Phase C の API 再設計と統合して実施）

#### B-3. cluster エラー型の階層化 [P1] [cluster] ✅ 調査完了（変更不要）
- [x] 23 個のエラー型を調査
- **結果**: エラー型は既にモジュール単位で適切に分離されており、各エラー型が固有のコンテキストを持つ
- **判断**: 現状の設計は「1 エラー型 = 1 責務」の原則に合致。無理な統合は情報損失のリスクあり。変更不要

#### B-4. PersistenceContext 隠蔽 [P1] [persistence] → Phase D に延期
- [x] 調査完了: PersistenceContext は Eventsourced trait の関連型として公開が必須
- **結果**: `type Context = PersistenceContext<...>` としてアクター定義で使用されており、trait 契約上 pub(crate) 化不可
- **延期理由**: trait 設計の根本的見直しが必要（Phase D の構造最適化で検討）

#### B-5. Remote エントリーポイント簡素化 [P1] [remote] → Phase C に延期
- [x] 調査完了: 7 階層の設定チェーンを分析（RemotingExtensionConfig → EndpointWriterConfig → ... → Transport）
- **結果**: RemotingSetup ビルダーで 60-75% の設定ボイラープレート削減が可能と推定
- **延期理由**: 新しいビルダー API の設計・実装が必要（Phase C で実施）

#### B-6. `persist_async` 改名 [P1] [persistence] ✅ 完了
- [x] `persist_async` → `persist_unfenced` にリネーム
- [x] rustdoc で命名理由を説明（「unfenced = コマンドスタッシュ/フェンシングなし」）
- [x] 全参照箇所を更新（89テスト通過確認）

#### B-7. cluster アーキテクチャガイド [P1] [cluster] [docs] ✅ 完了
- [x] Placement state machine 図を作成（Mermaid）
- [x] Membership coordinator flow を図示
- [x] Pub/Sub delivery guarantee を説明
- [x] `docs/guides/cluster-architecture.md` として配置
- [x] protoactor-go 対応表を記載

#### B-8. persistence Quickstart ガイド [P1] [persistence] [docs] ✅ 完了
- [x] `docs/guides/persistence-quickstart.md` を新規作成
- [x] Pekko EventSourcedBehavior との概念対応表
- [x] 最小限の永続アクター実装手順（8ステップ）
- [x] no_std 環境での使い方
- [x] Getting Started ガイドにリンク追加

---

### Phase C: 機能充実

#### C-1. TLS/SSL transport [P0] [remote]
- [ ] rustls ベースの `TokioTlsTransport` を設計
- [ ] `RemotingExtensionConfig::with_tls()` 設定 API を追加
- [ ] 証明書管理のユーティリティを提供
- [ ] TLS 有効時の example を追加
- [ ] テスト（自己署名証明書によるハンドシェイク検証）

#### C-2. Handshake timeout [P1] [remote]
- [ ] `RemotingExtensionConfig::with_handshake_timeout()` を追加
- [ ] timeout 超過時の EndpointAssociation FSM 遷移を実装
- [ ] recovery path のテスト

#### C-3. Stash メカニズム [P2] [actor]
- [ ] `ActorContext::stash()` / `unstash()` API を設計
- [ ] typed / untyped 両方に対応
- [ ] behavior transition 時のメッセージバッファリング実装
- [ ] テストと example 追加

#### C-4. Backpressure Protocol [P2] [actor]
- [ ] mailbox full 時の明示的ハンドリングを設計
- [ ] `on_mailbox_pressure()` hook または backoff integration
- [ ] テスト追加

#### C-5. 統合テスト拡充 [P2] [cluster] [remote]
- [ ] cluster: failure scenario テスト（node down, network partition, slow node）
- [ ] cluster: load-balanced placement 検証テスト
- [ ] remote: 2+ ノードシナリオテスト
- [ ] remote: Phi Failure Detector のエッジケーステスト

#### C-6. streams サンプル追加 [P1] [streams]
- [ ] 基本パターン 5 個程度の example を追加
  - [ ] Source → Sink の最小パイプライン
  - [ ] map / filter の基本変換
  - [ ] fold による集約
  - [ ] カスタム GraphStage
  - [ ] バックプレッシャーのデモ
- [ ] 各 example に概念説明コメントを付与

#### C-7. アーキテクチャドキュメント [P2] [docs]
- [ ] actor: Dispatcher → Mailbox → ActorCell の実行フロー図
- [ ] actor: no_std/std split の判定基準ドキュメント
- [ ] remote: FSM 状態遷移図、backpressure flow
- [ ] streams: GraphInterpreter の実装アルゴリズム説明
- [ ] 全体: モジュール間依存関係図

#### C-8. Behavior DSL の充実 [P1] [actor]
- [ ] `receive_and_reply` shorthand の追加
- [ ] message adapter builder の提供
- [ ] Behavior DSL のガイドドキュメント

---

### Phase D: 構造最適化

#### D-1. utils モジュール分離 [P2] [utils]
- [ ] fraktor-sync-rs 独立化の可行性調査（ArcShared, SharedAccess, InterruptPolicy）
- [ ] fraktor-concurrent-rs 独立化の可行性調査（AsyncBarrier, CountDownLatch, WaitGroup）
- [ ] 依存ツリー分析
- [ ] 分割実施（可行であれば）
- [ ] 全モジュールのビルド・テスト確認

#### D-2. Cluster Provider 拡充 [P2] [cluster]
- [ ] Kubernetes Cluster Provider の設計・実装
- [ ] Consul Cluster Provider の設計・実装（オプション）
- [ ] Provider trait の拡張性確認

#### D-3. Performance baseline [P3] [actor]
- [ ] `benches/` に criterion ベンチマーク整備
  - [ ] Actor spawn latency
  - [ ] Message send throughput
  - [ ] Dispatcher throughput
  - [ ] Scheduler tick 精度
- [ ] no_std vs std のオーバーヘッド比較

#### D-4. Persistent FSM [P2] [persistence]
- [ ] Pekko PersistentFSM に相当する trait 設計
- [ ] 状態遷移が明示的に定義される永続アクターパターン
- [ ] テストと example

#### D-5. JournalResponse 簡潔化 [P2] [persistence]
- [ ] 8 variant → Result ベースへの統一を検討
- [ ] 後方互換性は不要（CLAUDE.md の方針に従う）
- [ ] テスト更新

#### D-6. StageKind 命名規約 [P2] [streams]
- [ ] 将来の variant 増加に備えた命名パターンを策定
- [ ] 既存 variant をリネーム（必要に応じて）

#### D-7. ランタイム型チェック最小化 [P2] [streams]
- [ ] `DynValue + TypeId` の使用箇所を調査
- [ ] コンパイル時型チェックへ段階的に移行可能な箇所を特定
- [ ] no_std + alloc 制約下での実現可能性を検証

#### D-8. URI Parser の扱い [P3] [utils]
- [ ] actor 側に統合するか utils に残留するか判断
- [ ] 統合する場合は依存関係を更新

#### D-9. Router パターン [P3] [actor]
- [ ] roundrobin router の設計・実装
- [ ] scatter-gather router の設計・実装
- [ ] Pekko Router との対応確認

#### D-10. その他の改善
- [ ] remote: Flight Recorder 永続化（circular buffer / file sink）
- [ ] remote: Compression support
- [ ] cluster: Multi-Datacenter 対応（DC identifier, affinity）
- [ ] cluster: Consensus-based Placement
- [ ] persistence: Event adapter パターン
- [ ] streams: Pekko Streams 対応表の作成
- [ ] utils: 上位型エイリアス追加（`DefaultMutex<T>` 等）
