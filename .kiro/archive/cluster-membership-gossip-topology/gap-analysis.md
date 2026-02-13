# ギャップ分析: cluster-membership-gossip-topology

## 前提
- 要件は生成済みだが未承認のため、分析結果は要件調整の材料として扱う。

## 1. 現状調査（既存アセット）

### 主要コンポーネント
- Membership のモデル/状態: `modules/cluster/src/core/membership_table.rs`, `node_status.rs`
- Gossip のエンジン/状態: `modules/cluster/src/core/gossip_engine.rs`, `gossip_state.rs`, `gossip_event.rs`
- Cluster イベント: `modules/cluster/src/core/cluster_event.rs`
- トポロジとメトリクス: `modules/cluster/src/core/cluster_topology.rs`, `cluster_metrics.rs`, `cluster_core.rs`
- Provider: `modules/cluster/src/core/cluster_provider/local_cluster_provider.rs`, `static_cluster_provider.rs`
- std 拡張: `modules/cluster/src/std/local_cluster_provider_ext.rs`（Remoting lifecycle 連動）
- Quarantine と解決: `modules/cluster/src/core/identity_table.rs`
- Gossip ライフサイクル: `modules/cluster/src/core/gossiper.rs`, `noop_gossiper.rs`

### 観測されたパターン/制約
- `core` は no_std 前提。std 固有は `modules/cluster/src/std/*` に分離。
- EventStream 経由の `ClusterEvent::TopologyUpdated` が既存のトポロジ更新手段。
- `ClusterCore::apply_topology_*` がメトリクス更新/イベント発火の中核。
- Gossip と Membership のエンジンは存在するが、ランタイム/アクター/Transport 連携が未実装。

### 既存の統合ポイント
- `ClusterCore::on_topology` / `apply_topology_for_external` による TopologyUpdated 生成/反映
- `LocalClusterProviderGeneric::handle_connected` / `handle_quarantined` が remoting 連動の入口
- `IdentityLookup::on_member_left` が離脱の波及点

## 2. 要件対応マップ（Requirement-to-Asset Map）

| 要件 | 既存アセット | 充足状況 | ギャップ/備考 |
|---|---|---|---|
| 要件1: Membership/Gossip 基盤ライフサイクル | `ClusterCore::start_*`, `Gossiper` trait, `LocalClusterProviderGeneric::start_*` | **Partial** | Gossip の実体が Noop、Membership/Gossip アクター群の責務が未定義 |
| 要件2: MemberList 状態遷移/合意 | `MembershipTable`, `NodeStatus`, `MembershipEvent` | **Partial** | Suspect/Dead が未表現、合意フロー（Ack/再送/合流）が未実装 |
| 要件3: 失敗検知/隔離 | `MembershipTable::mark_heartbeat_miss`, `IdentityTable::quarantine` | **Partial** | ハートビート/タイマー/隔離解除の実運用が未実装 |
| 要件4: トポロジ更新生成/反映 | `LocalClusterProviderGeneric`, `ClusterCore::apply_topology_*`, `ClusterEvent::TopologyUpdated` | **Partial** | Membership/Gossip からの自動生成が未接続、バッチ化ルールが未定義 |
| 要件5: メトリクス/イベント連動 | `ClusterMetrics`, `ClusterEvent` | **Partial** | Membership/Gossip のイベント発火が未接続、タイムスタンプ付与の仕様が未定義 |

## 3. ギャップと制約

### 明確な不足（Missing）
- Membership/Gossip を運用する **アクター群**（役割・責務・起動/停止）自体が未実装。
- Gossip の **合意フロー**（diffuse/ack/reconcile）が実運用に接続されていない。
- 失敗検知の **周期/閾値/タイマー実装** が未定義。
- Quarantine の **再参加ルール/期限** が運用側に結び付いていない。
- Membership/Gossip の **EventStream 発火** が未実装（`MembershipEvent`, `GossipEvent` は定義のみ）。

### 仕様上の空白（Unknown/Decision Needed）
- `Join/Alive` と設計用語 `Joining/Up` の対応関係（現設計では Join=Joining / Alive=Up と定義）
- `Suspect/Dead/Leaving` と `NodeStatus` の対応関係（`Unreachable` を Suspect とみなすか等）
- Consensus の定義範囲（Ack ベースか、version/epoch ベースか、protoactor-go 準拠か）
- `TopologyUpdated` の **集約ウィンドウ/ハッシュ更新規則**
- EventStream に **時刻情報** をどう付与するか（既存イベント型の拡張が必要か）

### 既存制約
- no_std で動作する実装に限定される箇所が多い。
- 1ファイル1型・tests.rs などの lint 制約に従う必要がある。

## 4. 実装アプローチの選択肢

### Option A: 既存コンポーネント拡張
**概要**: `MembershipTable`/`GossipEngine` を `LocalClusterProviderGeneric` と `ClusterCore` に段階的に接続  
**利点**: 既存構成を活かせる、ファイル追加が少ない  
**欠点**: `LocalClusterProviderGeneric` が肥大化しやすく、責務が曖昧化するリスク

### Option B: 新規コンポーネント追加
**概要**: Membership/Gossip の専用アクター/サービス層を新設し、Provider はイベント経路のみ担当  
**利点**: 責務分離が明確、テスト容易  
**欠点**: 新規インターフェイス設計が必要、ファイル数が増える

### Option C: ハイブリッド
**概要**: `LocalClusterProviderGeneric` をトポロジ入口として維持しつつ、Membership/Gossip の内部運用は新設コンポーネントに分離  
**利点**: 段階移行が可能、既存 API 互換を維持しやすい  
**欠点**: 移行期に二重経路が発生しやすい

## 5. 複雑度/リスク評価
- **Effort**: L（1–2週間）  
  - Gossip 合意、状態遷移、EventStream/metrics 連動、no_std 制約の調整が必要
- **Risk**: Medium〜High  
  - 分散状態遷移の仕様決定とテスト設計の難度が高い

## 6. Research Needed（設計フェーズ持ち越し）
- protoactor-go の membership/gossip 仕様（state machine/ack/anti-entropy）
- Suspect/Dead と Unreachable/Removed の対応方針
- TopologyUpdated の集約条件とイベント時刻の扱い

## 7. 設計フェーズへの提案
- **推奨検討**: Option C（段階移行しながら責務分離を進める）
- **要決定事項**:
  - 状態遷移とイベント型の最終マッピング
  - EventStream へ出すイベント範囲（Membership/Gossip を含めるか）
  - no_std/ std でのタイマーと失敗検知の分離方針
