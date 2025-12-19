# PartitionIdentityLookup ギャップ分析

## 分析サマリー

- **スコープ**: `IdentityLookup` トレイトの分散ハッシュベース実装（protoactor-go の disthash 相当）
- **主要課題**: 現在の `IdentityLookup` トレイトに `get` / `remove_pid` メソッドが未定義
- **推奨アプローチ**: ハイブリッドアプローチ（トレイト拡張 + 新規コンポーネント作成）
- **既存コンポーネントの再利用率**: 約70%（VirtualActorRegistry, PidCache, RendezvousHasher）
- **主要リスク**: `IdentityLookup` トレイトの破壊的変更が必要

---

## 1. 現状調査

### 1.1 既存コンポーネント分析

| コンポーネント | ファイル | 再利用可否 | 備考 |
|---------------|---------|-----------|------|
| `IdentityLookup` | `/modules/cluster/src/core/identity_lookup.rs` | 拡張必要 | 現在 `setup_member/setup_client` のみ、`get/remove_pid` が未定義 |
| `NoopIdentityLookup` | `/modules/cluster/src/core/noop_identity_lookup.rs` | 参考 | 現在の実装パターンを参照 |
| `VirtualActorRegistry` | `/modules/cluster/src/core/virtual_actor_registry.rs` | **再利用可能** | アクティベーション管理、`ensure_activation`, `cached_pid`, `passivate_idle` 実装済み |
| `RendezvousHasher` | `/modules/cluster/src/core/rendezvous_hasher.rs` | **再利用可能** | オーナーノード選定 `select` 実装済み |
| `PidCache` | `/modules/cluster/src/core/pid_cache.rs` | **再利用可能** | TTL ベースキャッシュ、authority 無効化対応済み |
| `GrainKey` | `/modules/cluster/src/core/grain_key.rs` | **そのまま使用** | 仮想アクターキー |
| `VirtualActorEvent` | `/modules/cluster/src/core/virtual_actor_event.rs` | **そのまま使用** | Activated/Hit/Reactivated/Passivated 定義済み |
| `PidCacheEvent` | `/modules/cluster/src/core/pid_cache_event.rs` | **そのまま使用** | Dropped イベント定義済み |
| `ActivationError` | `/modules/cluster/src/core/activation_error.rs` | **そのまま使用** | SnapshotMissing/NoAuthority 定義済み |
| `ClusterCore` | `/modules/cluster/src/core/cluster_core.rs` | 統合ポイント | `identity_lookup` フィールド保持、トポロジ更新時に `pid_cache` 無効化あり |
| `ClusterExtensionInstaller` | `/modules/cluster/src/core/cluster_extension_installer.rs` | 統合ポイント | `with_identity_lookup` で差し替え可能 |

### 1.2 protoactor-go 参照実装との比較

| protoactor-go | fraktor-rs 現状 | ギャップ |
|---------------|----------------|---------|
| `IdentityLookup.Get(ClusterIdentity)` | 未定義 | **要追加** |
| `IdentityLookup.RemovePid(ClusterIdentity, PID)` | 未定義 | **要追加** |
| `IdentityLookup.Setup(cluster, kinds, isClient)` | `setup_member/setup_client` として分離済み | 対応済み |
| `IdentityLookup.Shutdown()` | 未定義 | 検討必要（要件外だが関連） |
| `disthash.Manager` | 該当なし | **新規作成必要** |
| `Manager.onClusterTopology` | `ClusterCore.apply_topology` あり | 統合方法の設計必要 |
| `Rendezvous.GetByClusterIdentity` | `RendezvousHasher::select` あり | 対応済み |

### 1.3 アーキテクチャパターン

```
現在のフロー:
  ClusterExtensionInstaller
    └── with_identity_lookup(ArcShared<dyn IdentityLookup>)
          └── ClusterCore
                ├── setup_member_kinds() → identity_lookup.setup_member()
                └── setup_client_kinds() → identity_lookup.setup_client()

要件後のフロー:
  ClusterExtensionInstaller
    └── with_identity_lookup(ArcShared<dyn IdentityLookup>)
          └── ClusterCore
                ├── setup_member_kinds() → identity_lookup.setup_member()
                ├── setup_client_kinds() → identity_lookup.setup_client()
                ├── get(GrainKey) → identity_lookup.get()      [新規]
                ├── remove_pid(GrainKey) → identity_lookup.remove_pid()  [新規]
                └── on_topology() → identity_lookup.update_topology()  [新規]
```

---

## 2. 要件実現可能性分析

### 2.1 要件マッピング

| 要件 | 対応状況 | 必要なアクション |
|------|---------|-----------------|
| **要件1**: IdentityLookup 実装 | 部分対応 | トレイト拡張 + 新規構造体作成 |
| **要件2**: Grain PID 解決 | コア機能あり | `get` メソッド追加、`VirtualActorRegistry` 統合 |
| **要件3**: オーナーノード選定 | **対応済み** | `RendezvousHasher::select` を使用 |
| **要件4**: PID キャッシュ統合 | **対応済み** | `PidCache` を内包 |
| **要件5**: VirtualActorRegistry 統合 | **対応済み** | 既存 `VirtualActorRegistry` を使用 |
| **要件6**: トポロジ変更対応 | 部分対応 | `update_topology`, `on_member_left` 追加 |
| **要件7**: PID 削除 | 未対応 | `remove_pid` メソッド追加 |
| **要件8**: アイドルパッシベーション | **対応済み** | `VirtualActorRegistry::passivate_idle` あり |
| **要件9**: ClusterCore 統合 | 部分対応 | トレイトメソッド追加後に統合コード追加 |
| **要件10**: 設定提供 | 未対応 | `PartitionIdentityLookupConfig` 新規作成 |
| **要件11**: イベント通知 | **対応済み** | `VirtualActorEvent`, `PidCacheEvent` を使用 |

### 2.2 ギャップタグ付け

| ギャップ | タグ | 詳細 |
|---------|------|------|
| `IdentityLookup` トレイトに `get` 未定義 | **Missing** | 破壊的変更が必要 |
| `IdentityLookup` トレイトに `remove_pid` 未定義 | **Missing** | 破壊的変更が必要 |
| `IdentityLookup` トレイトにトポロジ更新メソッド未定義 | **Missing** | `update_topology`, `on_member_left` 追加 |
| `PartitionIdentityLookup` 構造体が存在しない | **Missing** | 新規作成 |
| `PartitionIdentityLookupConfig` が存在しない | **Missing** | 新規作成 |
| 現在時刻の受け渡しパターン | **Constraint** | `no_std` 環境では外部から `now: u64` を渡す設計が必要 |
| `NoopIdentityLookup` のトレイト変更への追従 | **Constraint** | 既存実装の更新が必要 |

---

## 3. 実装アプローチ選択肢

### Option A: 既存コンポーネント拡張のみ

**適用条件**: 機能が既存構造に自然に収まる場合

**対象**:
- `IdentityLookup` トレイトにメソッド追加
- `NoopIdentityLookup` に stub 実装追加
- `ClusterCore` に統合ロジック追加

**トレードオフ**:
- ファイル数最小
- 既存パターンを活用
- `IdentityLookup` トレイトが肥大化
- `VirtualActorRegistry` との責務重複が発生

**評価**: 不適切。`IdentityLookup` と `VirtualActorRegistry` の責務が混在する。

---

### Option B: 新規コンポーネント作成

**適用条件**: 機能が明確に独立した責務を持つ場合

**新規ファイル**:
1. `/modules/cluster/src/core/partition_identity_lookup.rs` - メイン実装
2. `/modules/cluster/src/core/partition_identity_lookup_config.rs` - 設定
3. `/modules/cluster/src/core/partition_identity_lookup/tests.rs` - テスト

**既存ファイル更新**:
1. `/modules/cluster/src/core/identity_lookup.rs` - トレイト拡張
2. `/modules/cluster/src/core/noop_identity_lookup.rs` - stub 実装追加
3. `/modules/cluster/src/core.rs` - モジュールエクスポート追加

**トレードオフ**:
- 責務が明確に分離
- テストしやすい
- 既存コンポーネントの複雑化を防ぐ
- ファイル数増加
- インターフェース設計が慎重に必要

**評価**: 適切。protoactor-go の `disthash/identity_lookup.go` と同じ構造。

---

### Option C: ハイブリッドアプローチ（推奨）

**適用条件**: トレイト拡張と新規コンポーネントの両方が必要な場合

**フェーズ1: トレイト拡張**
- `IdentityLookup` トレイトに `get`, `remove_pid`, `update_topology`, `on_member_left`, `passivate_idle`, `drain_events`, `drain_cache_events` を追加
- `NoopIdentityLookup` に default 実装追加

**フェーズ2: 新規コンポーネント作成**
- `PartitionIdentityLookup` 構造体を新規作成
- 内部に `VirtualActorRegistry`, `PidCache`, authority リストを保持
- `PartitionIdentityLookupConfig` を新規作成

**フェーズ3: 統合**
- `ClusterCore` / `ClusterExtension` からの呼び出しパス確立
- トポロジ更新イベントの購読

**トレードオフ**:
- 段階的に実装可能
- 既存テストを維持しながら進行
- 将来の拡張性確保
- 計画が複雑
- 一貫性維持に注意が必要

**評価**: 最適。破壊的変更を最小限に抑えつつ、要件を満たす。

---

## 4. 実装複雑度とリスク評価

### 努力量: **M（3-7日）**

**根拠**:
- 新規パターンの導入あり（トレイト拡張）
- 既存コンポーネント（`VirtualActorRegistry`, `PidCache`）の再利用で効率化
- 統合テストが必要
- `no_std` 対応の確認が必要

### リスク: **Medium**

**根拠**:
- `IdentityLookup` トレイトの破壊的変更により、既存の実装（`NoopIdentityLookup`, テスト用 stub）すべての更新が必要
- `ClusterCore` との統合ポイントが複数あり、デッドロック回避の設計が必要（既存の `apply_topology_for_external` パターンを参照）
- protoactor-go の `Manager` が actor ベースで動作しているが、fraktor-rs では同期的な実装が適切か検討が必要

---

## 5. 設計フェーズへの推奨事項

### 5.1 推奨アプローチ

**Option C: ハイブリッドアプローチ** を採用

### 5.2 主要な設計判断事項

1. **トレイト設計**:
   - `IdentityLookup` に追加するメソッドのシグネチャ（特に `get` の戻り値: `Option<String>` vs `Result<String, LookupError>`）
   - デフォルト実装を提供するか、すべてのメソッドを必須にするか

2. **時刻管理**:
   - `no_std` 環境では `now: u64`（Unix タイムスタンプ秒）を外部から渡す設計
   - TTL 計算の精度と単位の決定

3. **authority リストの管理**:
   - `ClusterTopology` から `Vec<String>` を抽出するタイミング
   - joined/left の差分適用 vs 全量置換

4. **イベント取得パターン**:
   - `drain_events` のミュータブル性（`&mut self` が必要）
   - イベントバッファのサイズ制限

### 5.3 追加調査項目（Research Needed）

1. **protoactor-go の `placementActor` 相当の必要性**
   - protoactor-go では `ActivationRequest` を actor 経由で処理
   - fraktor-rs では同期的な `VirtualActorRegistry` で十分か確認

2. **`Shutdown` メソッドの必要性**
   - protoactor-go では `IdentityLookup.Shutdown()` あり
   - クリーンアップが必要なリソースがあるか確認

3. **スレッドセーフティ**
   - `PartitionIdentityLookup` の内部状態を `ToolboxMutex` で保護するパターン
   - `ClusterCore` と同様の設計が適切か確認

---

## 6. ファイル構成案

```
modules/cluster/src/core/
├── identity_lookup.rs              [更新] トレイトにメソッド追加
├── noop_identity_lookup.rs         [更新] stub 実装追加
├── partition_identity_lookup.rs    [新規] メイン実装
├── partition_identity_lookup/
│   └── tests.rs                    [新規] テスト
├── partition_identity_lookup_config.rs [新規] 設定
└── core.rs                         [更新] エクスポート追加
```

---

## 7. 依存関係マップ

```
PartitionIdentityLookup
├── implements IdentityLookup
├── contains VirtualActorRegistry
│   ├── uses RendezvousHasher
│   ├── contains PidCache
│   └── emits VirtualActorEvent
├── contains PidCache (direct for cache events)
│   └── emits PidCacheEvent
├── contains Vec<String> (authorities)
└── contains PartitionIdentityLookupConfig

ClusterCore
├── holds ArcShared<dyn IdentityLookup>
└── calls identity_lookup methods
    ├── setup_member()
    ├── setup_client()
    ├── get()           [新規]
    ├── remove_pid()    [新規]
    └── update_topology() [新規]
```

---

## チェックリスト

- [x] 要件とアセットのマッピング完了（ギャップタグ付け済み）
- [x] 3つの実装オプション提示（A/B/C）
- [x] 努力量（M）とリスク（Medium）の評価と根拠
- [x] 設計フェーズへの推奨事項
  - [x] 推奨アプローチ: Option C（ハイブリッド）
  - [x] 主要な設計判断事項
  - [x] 追加調査項目
