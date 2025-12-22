# ギャップ分析: cluster-api-identity-model

## 分析サマリ
- クラスタAPI本体（Get/Request/RequestFuture、Identityモデル）が未整備で、ClusterExtension/KindRegistry/IdentityLookupが“部品止まり”の状態。
- ActorSystem拡張取得の基盤は既存だが、クラスタ専用の取得導線や失敗時の振る舞いが不足。
- no_std前提は守られているため、APIはcoreに配置し、stdは補助に留める設計が必要。

## 前提・入力
- 仕様: `.kiro/specs/cluster-api-identity-model/requirements.md`
- ステアリング: `.kiro/steering/{product,structure,tech}.md`
- 現状コード: `modules/cluster/src` と `modules/actor/src/core`

## 1. 現状調査（資産・パターン・統合面）

### 既存資産（関連）
- **Cluster拡張/構成**: `core/cluster_extension.rs`, `core/cluster_extension_id.rs`, `core/cluster_extension_installer.rs`
- **Kind管理**: `core/kind_registry.rs`, `core/activated_kind.rs`
- **IdentityLookup基盤**: `core/identity_lookup.rs`, `core/partition_identity_lookup.rs`, `core/virtual_actor_registry.rs`
- **RPC/配送補助**: `core/grain_rpc_router.rs`, `core/serialized_message.rs`
- **ActorSystem拡張API**: `modules/actor/src/core/system/extended_actor_system.rs`

### 既存パターン/制約
- `core` は no_std 固定、`std` は feature 切替（`cfg-std-forbid`）
- 1ファイル1公開型、`tests.rs` 分離
- 共有は `ArcShared` 前提

### 統合面
- **ActorSystem拡張取得**: `ExtendedActorSystemGeneric::extension/register_extension` あり
- **Ask/Future基盤**: `ActorRefGeneric::ask`, `AskResponseGeneric`, `TypedAskFuture` など（actor/core）

## 2. 要件→資産マップ（ギャップ）

| 要件 | 既存資産 | 状態 | ギャップ |
|---|---|---|---|
| Identityモデル（kind/identity, key形式, 空文字禁止） | `GrainKey`, `ActivatedKind`, `KindRegistry` | **不足** | kind/identityを持つ型がなく、空文字検証もない |
| Cluster API取得/拡張登録 | `ClusterExtensionId`, `ClusterExtensionInstaller`, `ExtendedActorSystemGeneric` | **部分的** | 取得APIは汎用拡張のみ。クラスタ専用の取得導線/失敗モードが未定義 |
| kind登録とIdentityLookup準備 | `ClusterCore::setup_member_kinds/setup_client_kinds`, `KindRegistry` | **部分的** | 登録されていないkindの解決失敗は未整備 |
| PID解決（Get） | `IdentityLookup::get` / `PartitionIdentityLookup` | **不足** | Cluster API側のGetが存在せず、未起動時の失敗定義がない |
| Request/RequestFuture | Actor側に `ask` 基盤あり | **不足** | Cluster APIのRequest層が未実装 |
| no_std/std整合性 | `modules/cluster/src/core` と `std` 分離 | **部分的** | coreにAPIがないため、整合性要件を満たせない |

## 3. 追加で必要な機能（明示ギャップ）
- **ClusterIdentity型**（kind/identity + key生成、空文字検証）
- **Cluster API本体**（Get/Request/RequestFuture、未起動/未登録kindの失敗）
- **ActorSystem拡張取得のクラスタ専用ラッパ**
- **IdentityLookup連携の失敗ポリシー**
- **テスト追加**（要件ごとの最小カバレッジ）

## 4. 実装アプローチ案

### Option A: 既存部品の拡張（最小追加）
- 既存 `ClusterExtensionGeneric` に API を追加し、`ClusterIdentity` を core に新規追加
**長所**: 変更範囲が小さい  
**短所**: `ClusterExtension` が肥大化しやすい

### Option B: 新API層を新設
- `core/cluster_api.rs` を新設し、`ClusterExtension` は薄いファサードにする
**長所**: 責務分離が明確  
**短所**: ファイル数が増える

### Option C: ハイブリッド
- coreに `ClusterIdentity` と `ClusterApi` を追加し、`ClusterExtension` は取得導線のみ提供
**長所**: 拡張の肥大化を防ぎつつ既存拡張に統合  
**短所**: 設計の合意が必要

## 5. 見積もりとリスク
- **Effort**: M（3–7日）  
  既存部品はあるが、API設計・Identityモデル・エラーハンドリングとテストが必要
- **Risk**: Medium  
  no_std制約とActorSystem拡張の整合性が主なリスク

## 6. Research Needed（設計フェーズで調査）
- Cluster API の命名/責務（`ClusterExtension` に直接追加するか）
- `ClusterIdentity` と `GrainKey` の関係（互換保持か置換か）
- Request/RequestFuture の具体的な返却型とエラー型（Ask基盤との橋渡し）

