# リサーチログ: persistence-actor-api-hidden

## サマリー
- 既存の永続化拡張は ActorRef を公開する構造で、ActorSystem からの取得経路が不足している。
- ExtensionId + ExtensionInstaller による登録は serialization/cluster/remoting の共通パターンである。
- 永続化コンテキストと永続化状態を分離した合成パターンにより、ActorRef を公開APIから隠蔽できる。
- `PersistentActorBase` のような基底構造は不要で、明示的な状態と IO 境界の方が Rust の所有権モデルに合致する。
- persistence_id の単一ソースは Actor 側に固定するのが Pekko 互換である。
- `persistent_props` / `spawn_persistent` を唯一の入口にすることで Adapter を隠蔽できる。

## 調査ログ
### 1. 既存拡張ポイント
- `modules/actor/src/core/extension/*` と `ExtendedActorSystemGeneric` が extension 登録/取得の統一的な手段を提供している。
- `SerializationExtensionId` が ExtensionId の実装例であり、同様の登録/取得フローを踏襲できる。
- `ExtensionInstaller` が ActorSystem ブート時の拡張登録を担い、cluster/remoting も同様の経路を使用している。

### 2. 永続化基盤の現状
- `PersistentActorBase::new` が Journal/Snapshot の ActorRef を要求するため、ユーザAPIから隠蔽できていない。
- `PersistenceExtensionGeneric` は ActorRef を保持するが、ActorSystem からの取得経路が未整備。
- 基底構造に状態と API を閉じ込めると、合成・テスト・no_std 境界の観点で不利になる。
- `spawn_system_actor` が内部アクター生成の既存パターンであり、PersistenceExtension もこれに寄せるのが整合的。

### 3. 命名/設計指針
- `.codex/skills/avoid-ambiguous-suffixes` により、役割が曖昧なサフィックスを避ける必要がある。
- `.codex/skills/core-std-boundary` により、core 層で std 依存を導入しない構成が必須。

## アーキテクチャ判断の候補
- ExtensionId を追加し、`ActorSystem` の extension 機構から永続化拡張を取得する方式が最小の統合コスト。
- `PersistenceContext` と `PersistenceState` を分離し、`PersistentActor` は合成で利用する構成が本質的。
- `PersistentActorBase` は廃止し、状態と IO 境界を明示する。
- `PersistentActorAdapter` を設け、`pre_start` で初期化と recovery 開始を自動化する。
- `persistent_props` / `spawn_persistent` で Adapter を強制適用する。

## リスクと緩和
- **リスク**: Extension 未登録時に起動できない。
  - **緩和**: `pre_start` で検出し、明確なエラーで起動失敗とする。
- **リスク**: no_std と std の境界で初期化経路が分岐する。
  - **緩和**: core で完結する初期化 API を設計し、std 側は拡張登録のみ担う。
