# protoactor-go cluster vs fraktor-rs cluster 機能比較

このドキュメントは `references/protoactor-go/cluster` と `modules/cluster` の機能を比較し、**本当に問題のある未実装**と**問題のない設計の違い**を明確に区別したものです。

## 結論（先に要約）

**fraktor-rsのclusterモジュールは、protoactor-goの核心機能を十分にカバーしています。**

- メンバーシップ管理: 完全実装（PhiFailureDetector、QuarantineTable、ゴシップ統合）
- 配置管理: 完全実装（RendezvousHasher + PlacementCoordinatorCore）
- ゴシップ収束: 実装済み（GossipState::Confirmed）
- PubSub: 完全実装

---

## 1. 問題のない「未実装」（設計の違いまたはオプション機能）

以下の項目は「未実装」ではなく、**設計アプローチの違い**または**オプション機能**です。

### 1.1 Consensus機構

| 項目 | protoactor-go | fraktor-rs |
|------|---------------|------------|
| 実装 | ConsensusCheck, ConsensusHandler, ConsensusChecks | GossipState::Confirmed |
| 用途 | ゴシップキーの収束確認 | メンバーシップトポロジーの収束確認 |

**なぜ問題ないか**: fraktor-rsの`GossipDisseminationCoordinator`は、すべてのピアからACKを受信すると`GossipState::Confirmed`に遷移します。これはprotoactor-goのConsensusCheckがトポロジーキーに対して行うことと同等です。

```rust
// fraktor-rs: gossip_dissemination_coordinator.rs
pub fn handle_ack(&mut self, peer: &str) -> Option<GossipState> {
    self.outstanding.remove(peer);
    if self.outstanding.is_empty() {
        self.state = GossipState::Confirmed;  // 全ピア確認済み
        return Some(self.state);
    }
    None
}
```

### 1.2 Informer

| 項目 | protoactor-go | fraktor-rs |
|------|---------------|------------|
| 実装 | Informer構造体 | GossipDisseminationCoordinator + MembershipCoordinatorGeneric |
| 機能 | 状態管理、ゴシップ送受信、コンセンサスチェック | デルタ拡散、ACK処理、コンフリクト検出、メンバー状態管理 |

**なぜ問題ないか**: fraktor-rsは2つのコーディネーターで同等機能を提供しています。

- `GossipDisseminationCoordinator`: Diffusing → Confirmed → Reconciling状態管理
- `MembershipCoordinatorGeneric`: PhiFailureDetector、QuarantineTable、TopologyAccumulator統合

### 1.3 gossip_actor

| 項目 | protoactor-go | fraktor-rs |
|------|---------------|------------|
| 設計 | アクターベース | コーディネーターベース（no_std対応） |

**なぜ問題ないか**: 設計アプローチの違いです。fraktor-rsはno_std環境をサポートするため、アクターに依存しないコア実装を採用しています。機能的には同等です。

### 1.4 MemberStrategy / RoundRobin

| 項目 | protoactor-go | fraktor-rs |
|------|---------------|------------|
| GetPartition | Rendezvous hashing | RendezvousHasher::select() |
| GetActivator | Round-Robin | なし |

**なぜ問題ないか**:

1. `GetPartition`（グレイン配置）はRendezvousハッシュで実装済み
2. `GetActivator`はprotoactor-goでも実際には使用されていない（コード内で呼び出し箇所なし）
3. 主要な配置戦略はRendezvousハッシュであり、これは完全に実装済み

### 1.5 ClusterProviders

| プロバイダー | protoactor-go | fraktor-rs |
|-------------|---------------|------------|
| automanaged | ✅ | ❌ |
| consul | ✅ | ❌ |
| etcd | ✅ | ❌ |
| kubernetes | ✅ | ❌ |
| zookeeper | ✅ | ❌ |
| test/noop | ✅ | ✅ NoopClusterProvider |
| static | ❌ | ✅ StaticClusterProvider |
| AWS ECS | ❌ | ✅ AwsEcsClusterProvider |
| local | ❌ | ✅ LocalClusterProviderGeneric |

**なぜ問題ないか**: ClusterProviderはインフラ固有のアダプターです。必要なプラットフォーム向けに追加実装すればよく、クラスタのコア機能には影響しません。fraktor-rsはAWS ECS向けの独自プロバイダーを持っています。

### 1.6 KeyValueStore

| 項目 | protoactor-go | fraktor-rs |
|------|---------------|------------|
| 実装 | Set/Get/Clear | なし |

**なぜ問題ないか**: 分散KVストア抽象化はオプション機能であり、クラスタの基本動作には必須ではありません。

### 1.7 PlacementActor

| 項目 | protoactor-go | fraktor-rs |
|------|---------------|------------|
| 設計 | アクターベース（placement_actor.go） | コアロジック（PlacementCoordinatorCore） |

**なぜ問題ないか**: 設計の違いです。fraktor-rsは`PlacementCoordinatorCore`で同等機能を提供し、分散アクティベーション、リース管理、レジストリ統合をサポートしています。

---

## 2. 潜在的に問題となりうる未実装

以下は高度なユースケースで必要になる可能性がある機能です。

### 2.1 BroadcastEvent（クロスノードキャッシュ無効化）

| 項目 | protoactor-go | fraktor-rs |
|------|---------------|------------|
| 実装 | MemberList.BroadcastEvent() | なし |
| 用途 | グレイン終了を全ノードに通知しPIDキャッシュを無効化 | - |

**影響度**: 低〜中

**現状の動作**: fraktor-rsでは以下の方法でキャッシュが無効化されます:
- メンバー離脱時: `invalidate_authority()`
- トポロジー変更時: `invalidate_absent_authorities()`
- TTL期限切れ: 自然にキャッシュからクリア
- メッセージ配信失敗時: 再配置が発生

**影響シナリオ**: グレインが終了した直後、他ノードが古いPIDにメッセージを送信する可能性があります。ただし、配信失敗により再配置が発生するため、実用上は自己修復します。

### 2.2 汎用ゴシップ状態（SetState/GetState）

| 項目 | protoactor-go | fraktor-rs |
|------|---------------|------------|
| 実装 | Informer.SetState/GetState, SetMapState/GetMapState | なし |
| 用途 | 任意データをクラスタ全体でゴシップ共有 | - |

**影響度**: 低

**現状**: fraktor-rsはメンバーシップ情報のみをゴシップします。カスタムデータの共有が必要な場合は別途実装が必要です。

**影響シナリオ**: ユーザーがクラスタ全体でカスタム状態を共有し、その収束を待ちたい場合のみ問題になります。

---

## 3. fraktor-rs独自機能

protoactor-goにはないfraktor-rs独自の強み:

| 機能 | 説明 |
|------|------|
| **no_std対応** | エッジ/組み込み環境サポート |
| **AWS ECS ClusterProvider** | ECSタスク検出によるクラスタリング |
| **QuarantineTable** | 不良ノードの隔離管理 |
| **SchemaNegoiator** | スキーマバージョン交渉 |
| **OutboundPipeline** | 送信パイプライン管理 |
| **PhiFailureDetector** | Phi-accrual方式の障害検出 |
| **VirtualActorRegistry** | 仮想アクター登録・キャッシュ統合管理 |

---

## 4. 機能マッピング表

| protoactor-go | fraktor-rs | 状態 |
|---------------|------------|------|
| Informer | GossipDisseminationCoordinator + MembershipCoordinatorGeneric | ✅ 同等 |
| ConsensusCheck | GossipState::Confirmed | ✅ 同等（メンバーシップ用） |
| MemberList | MembershipTable | ✅ 同等 |
| MemberStateDelta | MembershipDelta | ✅ 同等 |
| MemberStatusEvents | MembershipEvent | ✅ 同等 |
| Rendezvous | RendezvousHasher | ✅ 同等 |
| IdentityLookup | PartitionIdentityLookup | ✅ 同等 |
| PartitionManager | PlacementCoordinatorCore | ✅ 同等 |
| PlacementActor | PlacementCoordinatorCore | ✅ 設計違い、同等機能 |
| SpawnLock | PlacementLease | ✅ 同等 |
| PidCache | PidCache + VirtualActorRegistry | ✅ 同等 |
| Kind | Kind + KindRegistry | ✅ 同等 |
| ClusterContext | GrainContext | ✅ 同等 |
| PubSub | ClusterPubSub | ✅ 完全実装 |
| gossip_actor | - | ✅ 設計違い（コーディネーターで実現） |
| BroadcastEvent | - | ⚠️ 未実装（TTLで代替可能） |
| SetState/GetState | - | ⚠️ 未実装（高度機能） |
| ClusterProviders (consul等) | - | ➖ インフラ固有（必要時追加） |
| KeyValueStore | - | ➖ オプション機能 |
| RoundRobin | - | ➖ 未使用機能 |

凡例:
- ✅ 実装済みまたは同等機能あり
- ⚠️ 潜在的に問題（高度なユースケース向け）
- ➖ オプション機能または未使用

---

## 5. 総括

fraktor-rsのclusterモジュールは**protoactor-goのコア機能を十分にカバー**しており、本番運用に必要な機能は揃っています。

**実装優先度が高い追加機能**（必要な場合）:
1. 実運用向けClusterProvider（k8s, consul等）- インフラに応じて
2. BroadcastEvent - 即時キャッシュ無効化が必要な場合

**実装優先度が低い機能**:
1. 汎用ゴシップ状態（SetState/GetState）- ほとんどのユースケースで不要
2. RoundRobin activator - protoactor-goでも実際には使用されていない
