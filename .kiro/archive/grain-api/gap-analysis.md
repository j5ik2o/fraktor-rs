# ギャップ分析: grain-api

## 分析サマリ
- ClusterApi/ClusterIdentity/IdentityLookup により最小限の参照解決と request API は存在するが、Grain API としての利用体験（Grain 参照型/実行コンテキスト/リトライ設定/観測イベント）が不足している。
- メッセージ表現は protobuf 固定ではない前提に対し、`SerializedMessage`/`GrainRpcRouter` 等の部品はあるが ClusterApi と統合されていない。
- 既存サンプルは Grain API を使った形になっておらず、要求される「参照取得+呼び出し」サンプルを新 API に合わせて整備する必要がある。

## 前提
- 要件は生成済みだが未承認のため、分析結果は要件調整の材料として扱う。

## 1. 現状調査（資産・パターン・統合面）

### 既存資産（関連）
- **クラスタAPI/参照解決**: `modules/cluster/src/core/cluster_api.rs`, `cluster_api_error.rs`, `cluster_request_error.rs`, `cluster_resolve_error.rs`
- **識別子モデル**: `modules/cluster/src/core/cluster_identity.rs`, `grain_key.rs`
- **配置/解決基盤**: `identity_lookup.rs`, `partition_identity_lookup.rs`, `placement_coordinator.rs`, `rendezvous_hasher.rs`, `pid_cache.rs`
- **仮想アクター管理**: `virtual_actor_registry.rs`, `virtual_actor_event.rs`
- **RPC/スキーマ**: `grain_rpc_router.rs`, `serialized_message.rs`, `schema_negotiator.rs`, `rpc_event.rs`, `rpc_error.rs`
- **観測基盤**: `cluster_event.rs`, `cluster_metrics.rs`, `pub_sub_event.rs`（EventStream 経由で配信する仕組みは存在）
- **サンプル**: `modules/cluster/examples/quickstart/main.rs`, `cluster_extension_tokio/main.rs`, `cluster_extension_no_std/main.rs`

### 既存パターン/制約
- `core` は no_std 前提で `std` 分岐を持たない（std 実装は `modules/cluster/src/std` に分離）。
- 1ファイル1型・`tests.rs` 分離などの lint 制約がある。
- 共有は `ArcShared<ToolboxMutex<...>>` を利用し、状態変更メソッドは `&mut self` が原則。
- 命名は `Manager/Service/Facade/Util/Runtime` 等の曖昧サフィックス禁止。

### 統合面
- **ActorSystem 拡張取得**: `ActorSystemGeneric::extended().extension_by_type` を通じて `ClusterExtensionGeneric` 取得済み。
- **Ask/Timeout**: `ClusterApiGeneric::request` が `ActorRef::ask` と `Scheduler` を使ったタイムアウト処理を持つ。
- **EventStream**: `ClusterExtension`/`ClusterCore` のイベントは Extension 経由で publish 済みだが、Grain 呼び出しに直結するイベントは未定義。

## 2. 要件→資産マップ（Requirement-to-Asset Map）

| 要件 | 既存資産 | 充足状況 | ギャップ/備考 |
|---|---|---|---|
| 要件1: Grain 識別と参照解決 | `ClusterIdentity`, `GrainKey`, `ClusterApiGeneric::get`, `ClusterCore::resolve_pid` | **Partial** | 参照解決は可能だが、解決結果に識別情報を保持する参照型がない。トポロジ更新中の挙動/一貫性を API として明示していない。 |
| 要件2: Grain 呼び出しと応答 | `ClusterApiGeneric::request/request_future`, `ClusterRequestError` | **Partial** | タイムアウトはあるがリトライ設定がない。配置未確定（`LookupError::Pending`）時の扱いが明示されていない。 |
| 要件3: Grain 実行コンテキスト | なし（ActorContext は存在） | **Missing** | `GrainContext` 相当の型・取得 API がない。 |
| 要件4: メッセージ表現と互換性 | `SerializedMessage`, `SchemaNegotiator`, `GrainRpcRouter` | **Partial** | シリアライザ登録/メッセージエンコードの入口が Grain API に存在しない。RPC ルータは独立部品で統合されていない。 |
| 要件5: 観測性と失敗通知 | `RpcEvent`, `VirtualActorEvent`, `ClusterEvent`, `ClusterMetrics` | **Partial** | Grain 呼び出し/失敗のイベントが EventStream に接続されていない。 |
| 要件6: 対応環境 | `core`/`std` 分離、`ClusterApi` が core | **Partial** | core に API はあるが、std 側の補助（タイムアウト/リトライ/サンプル）が整理されていない。 |
| 要件7: サンプルコード | 既存 examples | **Partial** | Grain API を使った「参照取得+呼び出し」を示すサンプルがない。既存例は手動配線。 |

## 3. ギャップと制約

### 明確な不足（Missing）
- **Grain 参照型**（identity を保持し get/request の入口になる型）
- **Grain 実行コンテキスト**（kind/identity/cluster 参照を取得する API）
- **リトライ設定**（protoactor-go の GrainCallConfig 相当の概念）
- **Grain 呼び出しイベントの観測経路**（EventStream への接続）

### 仕様上の空白（Unknown / Research Needed）
- Grain API を `ClusterApi` に統合するか新規層として分離するか
- メッセージ表現（`SerializedMessage`/`SchemaNegotiator`）を Grain API にどう接続するか
- 「配置未確定」の扱い（待機/即時失敗/リトライ）をどのレイヤで保証するか

### 既存制約
- `core` は no_std 固定、std 向け機能は `modules/cluster/src/std` に隔離が必要。
- 1ファイル1型/`tests.rs` など lint ルールに従う必要がある。
- 命名規約により `Manager/Service/Facade` などのサフィックスが使えない。

## 4. 実装アプローチの選択肢

### Option A: 既存 ClusterApi を拡張
**概要**: `ClusterApiGeneric` に Grain 参照型・リトライ・イベント連携を追加し、既存 API を拡張する。  
**利点**: 既存の利用導線を維持できる。  
**欠点**: `ClusterApi` が肥大化しやすく、責務境界が曖昧になる。

### Option B: 新規 Grain API 層を追加
**概要**: `core/grain_api.rs` などを新設し、`ClusterApi` は低レベル API として維持する。  
**利点**: Grain API の責務を明確化でき、設計が整理しやすい。  
**欠点**: 新規型の導入範囲が広がり、移行コストが増える。

### Option C: ハイブリッド
**概要**: `ClusterApi` は既存 get/request を維持し、`GrainRef`/`GrainContext` などを新設して上位層を構築する。  
**利点**: 既存 API を残しつつ Grain API の利用体験を改善できる。  
**欠点**: 2層 API の整合性設計が必要。

## 5. 複雑度/リスク評価
- **Effort**: L（1–2週間）  
  参照型/コンテキスト/リトライ/観測/サンプル更新を含み、設計項目が多い。
- **Risk**: Medium  
  no_std/std 境界、メッセージ表現の統合、API 層分割が主なリスク。

## 6. Research Needed（設計フェーズ持ち越し）
- Grain API の責務境界（`ClusterApi` 拡張 vs 新設）
- `SerializedMessage`/`SchemaNegotiator` を Grain API に組み込む方針
- リトライ/タイムアウト/未確定時の失敗ポリシー
- EventStream への Grain 呼び出しイベント設計

## 7. 設計フェーズへの提案
- **推奨検討**: Option C（既存 `ClusterApi` を維持しつつ Grain 参照型/コンテキストを追加）
- **要決定事項**:
  - Grain 参照型の API 形状と命名
  - メッセージ表現の接続点（既存 SerializationExtension との統合）
  - サンプルの更新方針（既存例の改修範囲）
