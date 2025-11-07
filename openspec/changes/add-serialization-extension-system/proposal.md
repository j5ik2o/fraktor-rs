# Serialization Extension System

## なぜ (Why)

現在のcellactor-rsには、Pekko/Akkaスタイルのプラガブルなシリアライゼーション機構が存在しません。分散アクターシステムでは、以下の要件を満たすシリアライゼーション機構が必要です：

1. **複数フォーマット対応**: JSON、CBOR、MessagePack、Protobuf など、用途に応じた最適なフォーマットを選択可能
2. **型ベースのルーティング**: メッセージ型ごとに適切なシリアライザを自動選択
3. **拡張可能性**: ユーザー定義のカスタムシリアライザを追加可能
4. **SystemGuardianとの統合**: アクターシステムのライフサイクルと連携
5. **no_std対応**: 組み込み環境でも動作する設計

Pekkoでは`SerializationExtension`として実装されており、これをRust/no_std環境に適合させた形で導入する必要があります。

## 何が変わるか (What Changes)

### 新規追加

- **Extension トレイトシステム**
  - `Extension<TB>` トレイト：ActorSystem ごとに1つのインスタンスを保証
  - `ExtensionId<TB>` トレイト：Extension の識別子とファクトリ
  - `ActorSystemGeneric<TB>` に Extension 管理機能を追加

- **Serialization Extension**
  - `SerializationExtension` / `SerializationExtensionId`：シングルトン識別子
  - `Serialization<TB>`：Pekko と同様に `serializer_id` を主キーとして `SerializedPayload` を構築し、`deserialize_payload` は `serializer_id → manifest` の順で解決する API を提供
  - `SerializerImpl`（低レベル） + `SerializerHandle`（高レベル）の二層 API により、`identifier()` / `serialize()` / `deserialize()` を実装者に要求しつつ、利用者には安全なインターフェイスを公開
  - `erased-serde` ベースのシリアライズ入力と、`Box<dyn Any + Send>` を返すデシリアライズ出力で Rust/no_std でも Pekko の Serializer SPI を再現
  - `TypeBinding`：型ごとに manifest 文字列、利用シリアライザ、型付きデシリアライザクロージャを保持し、ActorSystem 起動時に明示登録する
  - `SerializerRegistry`：`TypeId`, `(serializer_id, manifest)` および `serializer_id` の3テーブルを保有し、ID重複検出とローリングアップデート互換性を担保

- **Manifest / Payload ルーティング API**
  - `bind_type()` は `(serializer_id, manifest)` の組を一意に扱い、同じ組での再登録を `SerializationError::InvalidManifest` で拒否
  - `find_serializer_by_id` でシリアライザを取得したあとに manifest を手掛かりに TypeBinding を照合し、`deserialize_payload` で `Box<dyn Any>` を復元
- **Explicit Type Registration**
  - 送信時の暗黙登録やデフォルトフォールバックは禁止し、必要な型はすべて `bind_type()` 経由で登録するという Pekko 準拠モデルを採用

- **組み込みシリアライザ**
  - `BincodeSerializer`（組み込み）：高速バイナリシリアライザ
  - ActorSystem 初期化時に `register_serializer()` で ID=1 を予約登録し、型バインディングは利用側が明示的に行う
  - `JsonSerializer`（オプショナル）：人間可読なJSON形式
  - 将来的に MessagePack、CBOR、Protobuf をサポート

### 採用しなかった案（参考）

- `#[derive(Message)]` で `SERIALIZER_ID` を生成し、静的マッチングでシリアライザを選択する方式も検討したが、
  1. Extension/Serializer API を動的に追加・差し替えたいという Pekko 互換要件と矛盾する
  2. `no_std` + `alloc` を前提とした本プロジェクトにプロシージャルマクロ依存を追加するとビルド複雑度が増す
 ため、現行リリースでは採用しない。将来的なパフォーマンススパイクの候補として `openspec/changes/add-serialization-extension-system/design.md#q4` に記録済み。

### 変更される既存コンポーネント

- **ActorSystemGeneric**
  - Extension 登録・取得用のメソッド追加
  - システム起動時に `SerializationExtension` を自動登録

- **SystemStateGeneric**
  - Extension インスタンスの保存用フィールド追加（`HashMap<TypeId, Arc<dyn Any>>`）

### 影響範囲

- **破壊的変更**: なし（新規機能追加のみ）
- **影響するファイル**:
  - `modules/actor-core/src/system/base.rs`：Extension 管理メソッド追加
  - `modules/actor-core/src/system/system_state.rs`：Extension ストレージ追加
  - `modules/actor-core/src/serialization/`：新規モジュール群
  - `modules/actor-core/Cargo.toml`：依存クレート追加（bincode, serde など）

- **関連するspec**: なし（新規capability）
- **関連するchange**: `add-actor-root-guardian`（SystemGuardianとの統合）

## インパクト (Impact)

### ポジティブ

- 分散環境でのメッセージシリアライゼーションが可能に
- リモートアクター通信の基盤が整う
- Pekko互換性が向上し、移行が容易に
- 用途に応じた最適なシリアライザを選択可能

### リスクと軽減策

1. **no_std制約**
   - リスク：標準的なシリアライザ（serde_json等）はstd依存が強い
   - 軽減策：`alloc`ベースのシリアライザを選定（serde-json-core、postcard等）

2. **パフォーマンス**
   - リスク：シリアライゼーションのオーバーヘッド
   - 軽減策：bincode をデフォルトとし、ゼロコピー最適化を検討

3. **TypeId/manifest の安定性**
   - リスク：異なるビルドで TypeId が変わり、`serializer_id` と manifest がずれる
   - 軽減策：すべての型を `bind_type()` で明示登録し、`SerializedPayload` 側では `serializer_id` を主キーにしてから manifest を確認する。Rolling Update テストで旧 ID/新 ID の共存を継続検証する。

### マイグレーション

既存コードへの影響はなし（新規機能追加のみ）。将来的にリモートアクター機能を追加する際、自動的に SerializationExtension が使用されます。

## 実装ステップ (Tasks)

詳細は `tasks.md` を参照。

## 設計判断 (Design Decisions)

詳細は `design.md` を参照。
