# ギャップ分析: identity-lookup-placement

## 前提
- 要件は生成済みだが未承認のため、分析結果は要件調整の材料として扱う。

## 1. 現状調査（既存アセット）

### 主要コンポーネント
- IdentityLookup 抽象: `modules/cluster/src/core/identity_lookup.rs`
- 共有ラッパー: `modules/cluster/src/core/identity_lookup_shared.rs`
- パーティション型ルックアップ: `modules/cluster/src/core/partition_identity_lookup.rs`
- 設定: `modules/cluster/src/core/partition_identity_lookup_config.rs`
- ルックアップのコア構成要素:
  - 分散ハッシュ: `modules/cluster/src/core/rendezvous_hasher.rs`
  - アクティベーション管理: `modules/cluster/src/core/virtual_actor_registry.rs`
  - アクティベーション記録: `modules/cluster/src/core/activation_record.rs`
  - エラー/イベント: `modules/cluster/src/core/activation_error.rs`, `virtual_actor_event.rs`, `pid_cache_event.rs`
- Cluster 連携:
  - ルックアップ呼び出し: `modules/cluster/src/core/cluster_core.rs`（`resolve_pid`）
  - トポロジ更新反映: `cluster_core.rs`（`update_topology`/`on_member_left`）
  - 公開 API: `modules/cluster/src/core/cluster_api.rs`

### 観測されたパターン/制約
- `core` は no_std 前提で `std` 分岐を持たない（std 実装は `modules/cluster/src/std` に分離）。
- 1ファイル1型・tests.rs 配置などの lint 制約がある。
- 共有は `ArcShared<ToolboxMutex<...>>` を利用し、内部は `&mut self` 更新が原則。
- ルックアップは `IdentityLookup` トレイトで抽象化され、`PartitionIdentityLookup` は rendezvous hash で決定。
- 命名規約で `Manager/Service/Facade/Util/Runtime` など曖昧サフィックスは新規命名に使用できない。

### 既存の統合ポイント
- `ClusterCore::setup_*` が IdentityLookup の初期化を行う。
- `ClusterCore::apply_topology` が authority 更新と member 離脱を IdentityLookup に通知する。
- `ClusterApi` が `resolve_pid` を経由して PID を解決する。

## 2. 要件対応マップ（Requirement-to-Asset Map）

| 要件 | 既存アセット | 充足状況 | ギャップ/備考 |
|---|---|---|---|
| 要件1: Partition/Placement 解決と一貫性 | `IdentityLookup`, `PartitionIdentityLookup`, `RendezvousHasher`, `ClusterCore::apply_topology` | **Partial** | 配置決定はあるが「未確定時の拒否」やスナップショット公開は未実装。更新契約は authority 更新のみ。 |
| 要件2: 分散アクティベーションの単一性 | `VirtualActorRegistry`, `ActivationRecord` | **Partial** | ローカルな単一性はあるが、分散ロック/所有権の保証がない。 |
| 要件3: Lock/Storage/Activation 契約 | なし | **Missing** | Lock/Storage/Activation のトレイトや実装が存在しない。永続化や排他制御の抽象が不足。 |
| 要件4: ライフサイクルと観測性 | `ClusterCore::setup_*`, `VirtualActorEvent`, `PidCacheEvent` | **Partial** | start/stop の明示的な契約がない。イベントは生成されるが EventStream 等へ配信されていない。 |

## 3. ギャップと制約

### 明確な不足（Missing）
- PartitionManager / PlacementActor に相当する **分散協調コンポーネント** が未実装（名称は Manager 回避が必要）。
- **Lock/Storage/Activation** の契約（トレイト/インターフェイス）と実装が未定義。
- 分散アクティベーションの **排他保証**（ノード間の一意性）がない。
- ルックアップ/アクティベーションの **拒否理由** を返すエラー契約が未整備（`IdentityLookup::get` は `Option`）。
- 生成された `VirtualActorEvent`/`PidCacheEvent` の **観測経路** が未接続。
- 共有利用が必要な場合の **Shared ラッパー**（`*SharedGeneric`）の設計が未定義。

### 仕様上の空白（Unknown / Research Needed）
- PartitionManager/PlacementActor の **責務分解**（ルックアップ・ロック・永続化・起動の境界）。
- Lock/Storage の **実装方針**（no_std コアでの抽象、std 側の実装）。
- アクティベーション失敗時の **再試行/フォールバック** 方針。
- 既存 `IdentityLookup` API を維持するか、結果型を拡張するかの判断。
 - Manager サフィックス回避の **具体名**（例: `PartitionCoordinator`/`PlacementCoordinator`/`PlacementRouter` など）の検討。

### 既存制約
- core は no_std のため、ストレージやロック実体は std 側に分離する必要がある。
- 1ファイル1型・tests.rs の配置制約に従う必要がある。
- core に `cfg(feature = "std")` を入れられないため、境界は trait/実装分離で扱う必要がある。

## 4. 実装アプローチの選択肢

### Option A: 既存コンポーネント拡張
**概要**: `PartitionIdentityLookup` と `VirtualActorRegistry` に Lock/Storage の抽象を追加し、既存トレイトを拡張する。  
**利点**: 既存 API を活用しやすい、変更箇所が集中する。  
**欠点**: 単一コンポーネントが肥大化しやすく、責務分離が弱い。

### Option B: 新規コンポーネント追加
**概要**: PartitionManager 相当の新規コンポーネントを新設し（名称は Manager 回避）、Lock/Storage/Activation 契約を別トレイトとして設計する。  
**利点**: 責務分離が明確、テストと差し替えが容易。  
**欠点**: 既存 IdentityLookup との橋渡しが必要で設計コストが増える。

### Option C: ハイブリッド
**概要**: `IdentityLookup` は入口として維持し、分散協調は新規コンポーネント（名称は Manager 回避）に委譲する。  
**利点**: 既存 API 互換を保ちつつ、責務分離を進められる。  
**欠点**: 移行期間に二重経路が増えやすい。

## 5. 複雑度/リスク評価
- **Effort**: L（1–2週間）  
  - 分散アクティベーション、ロック/ストレージ契約、新規コンポーネントの導入が必要。
- **Risk**: Medium  
  - 既存 API との整合性維持と、no_std/std 境界設計が難所。

## 6. Research Needed（設計フェーズ持ち越し）
- PartitionManager/PlacementActor の **責務分割** とメッセージ契約。
- Lock/Storage 実装の **境界設計**（core の抽象、std 実装）。
- ルックアップ失敗の **エラー表現**（Option → Result の是非）。
- イベント観測の **配信経路**（EventStream 連動の設計）。
- `*SharedGeneric` を含む **共有ラッパー設計**（共有要否とハンドルの境界）。

## 7. 設計フェーズへの提案
- **推奨検討**: Option C（入口互換を保ちつつ分散協調を分離）
- **要決定事項**:
  - Lock/Storage/Activation の契約（責務・戻り値・失敗時処理）
  - Placement 決定の決定論（rendezvous hash を維持するか）
  - 観測イベントの出力先（EventStream か専用経路か）
  - Manager サフィックス回避の具体名（責務に応じた `*Coordinator`/`*Router`/`*Registry` 等）
