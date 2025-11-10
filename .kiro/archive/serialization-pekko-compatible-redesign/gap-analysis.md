# ギャップ分析統合レポート: Pekko互換シリアライゼーション機能再設計

> 本ドキュメントは、Claude分析とCodex分析を統合した包括的なギャップ分析レポートです。

---

## 📋 エグゼクティブサマリ

### 対象と目的
- **対象機能**: Pekko互換のシリアライゼーション機能再設計
- **既存コードベース**: cellactor-rs (no_std志向のActorランタイム)
- **参照実装**: Apache Pekko (Scala), protoactor-go (Go)
- **スコープ**: 既存のActorSystem/Extension機構を活用したシリアライゼーションレイヤーの新規構築

### 総合評価

| 指標 | 評価 | 詳細 |
|------|------|------|
| **工数** | **M (1週間)** | コアトレイト/レジストリ: 3-4日、ActorRef文字列化: 1-2日、組み込み: 2-3日、テスト: 2-3日 |
| **リスク** | **Medium** | manifest/エラーモデルの設計が最大のリスク、ActorRef文字列化は既存パターン踏襲で低リスク |
| **実現可能性** | **実装可能** | 既存Extension機構で基盤は整備済み、段階的実装により実現可能 |

### 主要な発見事項

#### ✅ 既存の強み
- **Extension機構は実装済み**: `Extension`/`ExtensionId`トレイトとレジストリ管理は完備
- **メッセージング基盤は存在**: `AnyMessage`による動的型メッセージシステムとTypeId活用
- **ActorSystem構造は成熟**: 初期化フック、状態管理、イベントストリームが整備済み
- **Runtime抽象**: `RuntimeToolbox`と`ArcShared`によりno_std/std兼用の同期primitivesを提供
- **再利用可能な基盤**: DeadLetter/EventStream、ActorPath/Pidなどの基礎インフラが揃っている

#### ❌ 主要なギャップ
- **シリアライザ抽象は未実装**: Pekko互換のSerializer/Registry/ManifestSerializerが存在しない
- **ActorRef文字列化ヘルパーなし**: `serialized_actor_path`ヘルパーとスコープ管理APIが未定義
- **レジストリ統制機構なし**: SerializerId/予約域、衝突検知、SerializationSetup/Builder DSLが未定義
- **マニフェスト管理なし**: SerializedMessage、SerializerWithStringManifest、型進化ロジックが欠落
- **組み込みシリアライザなし**: Null/Primitive/String/Bytes/ActorRefの標準実装が存在しない

#### ⚠️ 技術的課題
- **manifest/エラーモデル設計**: 後続のRemoting/Persistenceに影響する設計判断が必要
- **ActorRef文字列化統合**: 既存ActorPath実装との統合要検討（既存パターン踏襲で低リスク）
- **Serde非依存保証**: 仕様ドキュメント/Builderでの明示的な非依存契約が未整備

---

## 1. 現状調査結果

### 1.1 既存アセット

#### Extension機構 (`modules/actor-core/src/extension/`)
- **`Extension<TB>`トレイト**: マーカートレイト、`Send + Sync + 'static`境界
- **`ExtensionId<TB>`トレイト**: ファクトリパターン、`create_extension(&ActorSystemGeneric<TB>)`メソッド
- **レジストリ統合**: `ActorSystem::register_extension()`, `extension()`, `has_extension()`が実装済み
- **保存先**: `SystemStateGeneric::extensions` (TypeIdキーのHashMap)
- **TODO残存**: `ActorSystemGeneric::bootstrap`でシリアライゼーション拡張を有効化するTODOが残っている

#### メッセージング (`modules/actor-core/src/messaging/`)
- **`AnyMessageGeneric<TB>`**: `ArcShared<dyn Any + Send + Sync>`でペイロードを保持
- **`AnyMessageView`**: メッセージビューによる型安全なアクセス
- **TypeId活用**: `payload.type_id()`でデバッグ情報を取得可能、型解決の基礎として利用可能
- **SystemMessage**: Create/Recreate/Failure/Terminated等のライフサイクルメッセージが優先処理
- **エラーハンドリング**: `SendError`/`DeadLetterReason`による配投/観測の標準化

#### ActorSystem基盤
- **初期化フック**: `new_with()`で設定コールバックを受け取り、guardian起動前に実行
- **状態管理**: `SystemStateGeneric`がセル/名前解決/メトリクス/Extensionを一元管理
- **EventStream**: DeadLetter/Log/ライフサイクルイベントを購読可能
- **ActorPath/Pid**: ローカル監視とDeadLetterを支える基礎実装が存在

#### Runtime抽象 (`modules/utils-core/src/runtime_toolbox/`)
- **`RuntimeToolbox`**: no_std/std兼用の同期primitivesを提供
- **`ArcShared`**: 共有所有権の抽象化、メッセージ共有の基盤
- **依存関係**: Cargo依存に`postcard`/`prost`/`serde`/`bincode`等のシリアライザ候補が既に含まれる

### 1.2 Pekko参照実装の特徴

#### トレイト階層
```scala
Serializer (identifier, toBinary, fromBinary, includeManifest)
├─ SerializerWithStringManifest (manifest(o), fromBinary(bytes, manifest))
├─ ByteBufferSerializer (toBinary(o, buf), fromBinary(buf, manifest))
└─ AsyncSerializer (toBinaryAsync, fromBinaryAsync)
```

#### レジストリ構造
- **SerializationSetup**: プログラム的にシリアライザと型バインディングを登録
- **Serialization Extension**: ActorSystemへの登録、型→シリアライザ解決、スコープ管理
- **識別子管理**: 0-40は予約域、衝突検知とconfig/プログラムの優先順位制御

#### ActorRef文字列化サポート
- **スコープ管理**: `withTransportInformation` でコンテキスト設定・復元（Serialization.scala:114-122）
- **ヘルパー関数**: `serializedActorPath()` がActorPathを適切な文字列形式に変換（Serialization.scala:70-93）
- **フォールバック**: リモートアドレス情報がない場合はローカルパスを返す

### 1.3 protoactor-go参照実装の特徴

#### シンプルなインターフェース
```go
type Serializer interface {
    Serialize(msg interface{}) ([]byte, error)
    Deserialize(typeName string, bytes []byte) (interface{}, error)
    GetTypeName(msg interface{}) (string, error)
}
```

#### グローバルレジストリ
- **`RegisterSerializer()`**: init()でprotobuf/jsonシリアライザを登録
- **`Serialize(message, serializerID)`**: IDでシリアライザを選択し、typeName付き結果を返す
- **エラーハンドリング**: ID範囲外でエラー返却、シンプルな失敗通知

### 1.4 プロジェクト固有の規約と制約

#### 構造ルール
- **1ファイル1型**: 複数の構造体・traitを1ファイルに記述しない
- **mod.rs禁止**: 2018モジュールシステムを使用
- **テスト配置**: `hoge.rs`に対して`hoge/tests.rs`に単体テストを配置
- **ドキュメント**: rustdoc(`///`, `//!`)は英語、その他のコメント・ドキュメントは日本語

#### 技術方針
- **no_stdファースト**: ランタイム本体で`#[cfg(feature = "std")]`による機能分岐を入れない（テストコード内は許容）
- **参照実装**: protoactor-go(Go)、Pekko(Scala)の実装を参考にGoイディオムからRustイディオムに変換
- **破壊的変更許容**: 正式リリース前の開発フェーズのため、最適な設計を優先
- **Serde非依存**: フレームワーク非依存を推奨、Serdeはoptional依存

#### 品質基準
- **テスト完全性**: すべてのテストがパスすること、コメントアウトや無視は禁止
- **CI検証**: `./scripts/ci-check.sh all`で最終検証（途中工程では対象範囲のテストに留めてよい）
- **一貫性**: 既存の多くの実装を参考にして一貫性のあるコードを書く

---

## 2. 要求仕様と既存実装のマッピング

### Requirement 1: シリアライザ識別子とレジストリ統制

| 要求事項 | 既存の対応状況 | ギャップ | 優先度 |
|---------|--------------|---------|--------|
| プログラム登録優先 | ❌ レジストリ未実装 | **SerializationSetup/Builder DSL全体の新規構築** | 🔴 Critical |
| 識別子衝突検知 | ❌ 識別子概念なし | **SerializerId型と0-40予約域チェックロジック** | 🔴 Critical |
| TypeId完全一致→トレイトタグ→フォールバック | ⚠️ TypeIdのみ利用可能 | **トレイトマーカー/明示バインディング解決ロジック** | 🟡 High |
| NotSerializableエラー | ⚠️ Resultベースのエラーは可能 | **SerializationError型とメタデータ構造** | 🟡 High |
| 起動フェーズでの構築フロー | ⚠️ Extension登録は可能 | **ActorSystem初期化時の依存関係管理** | 🟡 High |

**実装インパクト**: M (2-3日)、既存Extension機構を活用可能だが、Builder DSL設計が必要

### Requirement 2: マニフェスト互換と型進化

| 要求事項 | 既存の対応状況 | ギャップ | 優先度 |
|---------|--------------|---------|--------|
| マニフェスト取得とSerializedMessage格納 | ❌ SerializedMessage未実装 | **SerializedMessage構造体とencode/decode** | 🔴 Critical |
| 未登録マニフェストでエラー | ❌ マニフェスト概念なし | **SerializerWithStringManifest実装とバージョン管理** | 🟡 High |
| 複数バージョン互換ロジック | ❌ 進化ロジックなし | **優先順位リストと試行ロジック、manifest キャッシュ** | 🟢 Medium |
| ローカル/永続化での自動適用 | ❌ リモート/ローカル区別なし | **ActorRef文字列化統合設計** | 🟡 High |
| bytes依存の選定 | ⚠️ Cargo依存に候補あり | **`bytes`クレートまたは代替バッファのno_std対応** | 🟡 High |

**実装インパクト**: M (2-3日)、バイトフォーマット仕様の明確化とキャッシュ戦略の設計が必要

### Requirement 3: ActorRef文字列化とExtension統合

| 要求事項 | 既存の対応状況 | ギャップ | 優先度 |
|---------|--------------|---------|--------|
| ActorRef文字列化のためのスコープ管理API | ❌ スコープAPI未実装 | **`with_serialization_scope`スコープAPI、`serialized_actor_path`ヘルパー** | 🟡 High |
| スコープ外アクセスでエラー | ⚠️ エラー返却は可能 | **スコープチェックとエラーパス、fallback仕様** | 🟡 High |
| ActorRef文字列化ヘルパー | ⚠️ ActorPath文字列化は部分的 | **`serialize_actor_ref`専用APIと統合** | 🟡 High |
| シャットダウン時のクリア | ⚠️ Extensionシャットダウンは未定義 | **Extension停止フックと状態リセット** | 🟢 Medium |
| thread-local代替設計 | ❌ no_stdでの実装未検討 | **Tokio/embassy両対応のハンドル設計とAPI境界** | 🔴 Critical |

**実装インパクト**: L (2-3日)、**High Risk** - no_stdでのtask-local相当の実装が未確定

### Requirement 4: 組み込みシリアライザラインアップ

| 要求事項 | 既存の対応状況 | ギャップ | 優先度 |
|---------|--------------|---------|--------|
| 組み込みシリアライザ登録 | ❌ 組み込みなし | **Null/Primitive/String/Bytes/ActorRefの標準実装** | 🔴 Critical |
| デフォルトシリアライザ自動登録 | ❌ 自動登録なし | **起動時の自動バンドル登録戦略** | 🟡 High |
| シンプルバッファAPI | ⚠️ Cargo依存に候補あり | **Vec<u8>または固定長バッファの基本実装** | 🟡 High |

**実装インパクト**: S (2-3日)、組み込みシリアライザの実装は明確

**スコープアウト**:
- **AsyncSerializer**: 将来の拡張として検討（Phase 4以降）、初期実装では同期のみで十分
- **ByteBufferSerializer（ゼロコピー）**: 将来の最適化として検討、Phase 1はシンプルバッファで十分

### Requirement 5: Serde非依存の仕様定義

| 要求事項 | 既存の対応状況 | ギャップ | 優先度 |
|---------|--------------|---------|--------|
| Serde非依存トレイト定義 | ✅ `dyn Any`ベースで可能 | **低リスク: トレイト設計で保証** | 🟢 Low |
| Serde専用シリアライザ任意登録 | ⚠️ レジストリ実装次第 | **Builder DSLで選択可能な構造** | 🟢 Medium |
| no_stdでのSerde未リンク | ✅ no_stdファースト設計 | **Feature gating、CIターゲットによるビルドバリア** | 🟡 High |
| API契約の明示 | ❌ 記述なし | **仕様ドキュメント/Builderでの明示的な非依存契約** | 🟢 Medium |

**実装インパクト**: S (1日)、設計・ドキュメント作業が中心

### Requirement 6: ネスト型の再帰委譲

| 要求事項 | 既存の対応状況 | ギャップ | 優先度 |
|---------|--------------|---------|--------|
| フィールドシリアライズ委譲API | ❌ Serialization API未実装 | **Serialization::serialize()の公開と委譲ルール** | 🟡 High |
| 未登録シリアライザでエラー | ⚠️ エラー返却は可能 | **委譲時のエラー伝播ガイド** | 🟢 Medium |
| Manifest/スコープ情報共有 | ❌ 情報スコープ未実装 | **親呼び出しからの自動引き継ぎ** | 🟡 High |
| 再帰委譲の基礎 | ✅ `ArcShared`による再利用可能 | **低リスク: メッセージ共有機構は存在** | 🟢 Low |

**実装インパクト**: M (1-2日)、再帰呼び出しとエラー伝播の設計が必要

---

## 3. 実装アプローチの選択肢

### Option A: actor-core 既存コンポーネント拡張

**想定**: `modules/actor-core/src/serialization/` ディレクトリを新設し、既存のExtensionパターンに従って実装

#### 拡張対象
- **ActorSystem初期化**: `new_with()`コールバックでSerializationExtensionを登録
- **SystemStateGeneric**: `extensions`マップで`SerializationExtension`を管理
- **AnyMessage**: `serialize()`/`deserialize()`ヘルパーメソッドを追加（オプショナル）

#### 新規ファイル構成例
```
modules/actor-core/src/serialization/
├─ serialization.rs              # Extension本体、Registry管理
├─ serializer.rs                 # Serializerトレイト群
├─ serializer_id.rs              # SerializerId型、予約域チェック
├─ serialized_message.rs         # SerializedMessage構造体、encode/decode
├─ serialization_setup.rs        # Builder DSL、バインディング登録
├─ serialization_error.rs        # エラー型定義
├─ serialization_context.rs      # ActorRef文字列化スコープ管理
└─ builtin/                      # 組み込みシリアライザ
   ├─ null_serializer.rs
   ├─ primitive_serializers.rs
   ├─ string_serializer.rs
   ├─ bytes_serializer.rs
   └─ actor_ref_serializer.rs
```

#### トレードオフ
- ✅ **既存パターン踏襲**: Extension登録フローが明確で学習コスト低
- ✅ **段階的導入**: コア機能への影響を最小限に保ちながら追加可能
- ✅ **no_std適合**: 既存の`no_std`制約と`RuntimeToolbox`抽象に適合しやすい
- ✅ **統合容易性**: Extension/DeadLetter/EventStreamと密に統合しやすい
- ⚠️ **ByteBufferSerializerの依存**: Phase 1では`Vec<u8>`でシンプルに実装、Phase 3以降でno_std対応の軽量バッファを検討
- ❌ **責務肥大化**: actor-coreが肥大化し、将来のRemoting/Persistence増設時に責務過多となる恐れ

### Option B: serialization-core 新規クレート

**想定**: `modules/serialization-core`（仮）を別クレートとして分離し、`actor-core`から疎結合で参照

#### 構成
- **`cellactor-serialization-rs`**: Pekko互換トレイトとレジストリ実装
- **`actor-core`側の統合点**: `SerializationExtension`が新クレートを薄くラップ
- **依存方向**:
  - `serialization-core` → `utils-core` (Runtimeツール利用)
  - `actor-core` → `serialization-core` (Optional feature)

#### トレードオフ
- ✅ **責務分離**: シリアライゼーションロジックがActorシステムから独立、明快な責務分離
- ✅ **再利用性**: 他のプロジェクトがSerializationクレート単独で利用可能
- ✅ **疎結合**: actor-stdや将来のTransportクレートと疎結合にできる
- ❌ **複雑性増加**: クレート境界でのAPIデザインとビルド管理の負担、workspace依存関係の再整理
- ❌ **統合テスト**: ActorSystemとの統合を別途検証する必要
- ❌ **初期コスト**: クレート追加・API設計・テストの初期コストが大きい
- ⚠️ **no_std維持**: 分離クレートもno_stdファーストで設計する必要

### Option C: Hybrid (core + std hook) - **推奨**

**想定**: Option Aをベースに、将来的なクレート分離を視野に入れたモジュール境界を設計し、ActorRef文字列化は段階的に対応

#### 実装戦略
1. **Phase 1 (MVP)**: `actor-core/src/serialization/` で基本実装（Requirement 1, 4）
   - Serializerトレイト群（同期のみ）
   - SerializationSetup、Builder DSL
   - 組み込みシリアライザ（Null/Primitive/String/Bytes/ActorRef）
   - ActorRef文字列化ヘルパー基本実装
   - シンプルバッファAPI（Vec<u8>）
2. **Phase 2 (互換)**: Pekko互換性の完成（Requirement 2, 3, 6）
   - SerializerWithStringManifest、SerializedMessage
   - マニフェスト管理とキャッシュ
   - ActorRef文字列化統合
   - ネスト委譲API
   - バージョン互換ロジック（オプション）
3. **Phase 3以降（将来検討）**: 利便性向上と最適化
   - std環境でのスレッドローカル対応（オプション）
   - AsyncSerializer（将来検討）
   - ByteBufferSerializerゼロコピー最適化（将来検討）
   - 独立クレート化（必要に応じて）

#### 段階的マイルストーン
- **M1 (MVP、Phase 1)**: Serializerトレイト、SerializationSetup、組み込みシリアライザ、ActorRef文字列化ヘルパー
- **M2 (互換、Phase 2)**: SerializerWithStringManifest、SerializedMessage、マニフェスト管理、ActorRef文字列化統合、ネスト委譲API

#### トレードオフ
- ✅ **段階的リスク低減**: 各マイルストーンで動作確認し、設計を調整可能
- ✅ **既存コード影響最小**: Extension機構を活用し、コア変更を抑制
- ✅ **将来の柔軟性**: モジュール境界を明確にすることで、後のクレート分離が容易
- ✅ **プラットフォーム差吸収**: no_std/std環境の差を段階的に吸収
- ✅ **短期実装可能**: Phase 1-2で基本機能を提供し、利便性向上は後回し
- ❌ **初期設計負荷**: 将来拡張を見越した抽象化で、初期実装が若干複雑化
- ⚠️ **スレッドローカル最適化**: Phase 3でのstd環境対応が技術的チャレンジ
- ⚠️ **段階的リリース計画**: API分割と調整が複雑

---

## 4. 実装複雑度とリスク評価

### 全体評価
- **工数**: **M (1週間)**
  - Phase 1 (MVP、必須): 4-5日
  - Phase 2 (互換、必須): 3-4日
  - テスト/ドキュメント: 2-3日（各Phase）
  - Phase 3以降（オプション、将来検討）: 実装しない
- **リスク**: **Medium**
  - ActorRef文字列化ヘルパーは Low Risk（既存パターン踏襲で低リスク）
  - シンプルバッファAPI (Vec<u8>) は Low Risk（標準的な実装）
  - manifest/エラーモデルの設計が最大のリスク要因（後続のRemoting/Persistenceに影響）
  - AsyncSerializer/ゼロコピー最適化はスコープアウト（将来検討）

### コンポーネント別評価

| コンポーネント | 工数 | リスク | 理由 | Phase |
|--------------|------|--------|------|-------|
| Serializerトレイト群 | S (1日) | Low | Pekkoパターン踏襲、dyn Any活用で実現可能 | Phase 1 |
| SerializerId/予約域 | S (半日) | Low | 単純な整数型と範囲チェック | Phase 1 |
| SerializationSetup/Builder | S (1-2日) | Low | 既存のBuilder/DSLパターンを参考に可能 | Phase 1 |
| SerializedMessage | S (1日) | Low | 構造体とencode/decode、バイトフォーマット仕様明確 | Phase 1 |
| TypeIdベース解決 | S (1日) | Low | HashMapでTypeId→Serializerマッピング | Phase 1 |
| Extension統合 | S (1日) | Low | 既存パターン踏襲で低リスク | Phase 1 |
| 組み込みシリアライザ | S (2-3日) | Low | Null/Primitive/String/Bytes/ActorRefの実装明確 | Phase 1 |
| マニフェスト管理 | M (2日) | Medium | バージョン互換ロジック、manifest キャッシュの設計要検討 | Phase 2 |
| **ActorRef文字列化ヘルパー** | **S (1日)** | **Low** | **ヘルパー関数とスコープAPI、Pekkoにもフォールバックあり、API設計明確** | **Phase 1 (必須)** |
| ActorRef文字列化統合 | M (1-2日) | Medium | 既存ActorPath実装との統合要検討 | Phase 2 |
| ネスト委譲API | M (1-2日) | Medium | 再帰呼び出しとエラー伝播の設計 | Phase 2 |
| バージョン互換ロジック | M (1-2日) | Medium | 優先順位リストと試行ロジック、Rust所有権モデルへのマッピング | Phase 2 (オプション) |

**Phase 3以降（オプション、将来検討）**:
| スレッドローカル最適化 | M (2日) | Medium | std環境での利便性向上、実装パターンは明確 | Phase 3 (オプション) |
| AsyncSerializerトレイト | M (2日) | Medium | async_trait利用、非同期境界の設計 | Phase 4 (将来検討) |
| ByteBufferSerializer（ゼロコピー） | M (2日) | Medium | no_stdでのゼロコピー最適化 | Phase 4 (将来検討) |

### 技術的課題と調査事項

#### 🟢 Low Risk: ActorRef文字列化ヘルパー（Phase 1-2、必須）
- **なぜ必要か**: ActorRefをシリアライズする際、リモートノード間でのActorの識別と通信を可能にするため、ActorPathを適切な文字列形式（完全修飾パスまたはローカルパス）に変換する必要がある。リモート通信において、送信先ノードがActorを解決できる形式でパスを提供することが必須。
- **アプローチ**: Pekko互換の設計（Serializerトレイトはクリーンなまま）
- **理由**: Pekkoにもリモートアドレス情報がnullの場合のフォールバック処理が存在（Serialization.scala:76-82）
- **実装方針**:
  - **Serializerトレイトはクリーン**（Pekko: Serializer.scala:65参照）
  - `SerializationExtension`が`serialized_actor_path(actor_ref)` ヘルパーを提供（Pekko: Serialization.scala:70-93参照）
  - Serializer実装がActorRefをシリアライズする際、このヘルパーを呼び出す
  - ヘルパーがExtension内の状態を参照し、ない場合はローカルパスのみ返す
  - `with_serialization_scope` スコープAPIで状態を設定・管理（Pekko: Serialization.scala:114-122参照）
- **利点**: Pekko互換、Serializerトレイトがクリーン、no_std対応可能
- **推奨**: Phase 1-2でExtension内の状態管理、将来的にstd環境でThreadLocal対応を検討

#### 🟡 Medium Risk: スレッドローカル最適化（Phase 3以降、オプション）
- **なぜ必要か**:
  - **利便性向上**: Phase 1-2のExtension内状態管理では、ネストしたシリアライズ呼び出しのたびに`SerializationExtension`を明示的に渡す必要があり、コードが冗長になる
  - **パフォーマンス改善**: ThreadLocalを使うことで、スコープ内で暗黙的にコンテキストにアクセスでき、関数呼び出しオーバーヘッドを削減
  - **Pekko互換性**: Pekkoの`DynamicVariable`（ThreadLocal）による使いやすいAPIと同じ開発者体験を提供
  - **例**: `serialized_actor_path(actor_ref)`が内部的にThreadLocalから情報を取得するため、呼び出し側はExtensionを意識しなくてよい
- **目的**: `with_serialization_scope`スコープAPIの内部実装をThreadLocalで最適化（SerializerトレイトのAPIは変わらない）
- **Phase 3の対応**: std環境でのThreadLocal実装（`modules/actor-std`で提供）
- **実装方針**:
  - `thread_local!`でスコープ情報を保持（Pekko互換）
  - `with_serialization_scope`スコープAPIが内部でThreadLocalを使用
  - `serialized_actor_path`ヘルパーがThreadLocalから情報を取得
  - no_std環境ではExtension内の状態管理を継続使用
- **重要**: SerializerトレイトのAPIは変わらない。内部実装の最適化のみ。
- **将来の検討事項**（実装不要）:
  - embassy対応時のtask-local相当（必要になれば検討）
- **推奨**: Phase 3以降の最適化として検討、Phase 1-2のExtension内状態管理で十分

#### 🟢 Low Risk: シンプルバッファAPI（Phase 1、必須）
- **なぜ必要か**: シリアライズ結果を保持するバイト配列が必要。Phase 1では実装の複雑性を最小限に抑えるため、標準的な`Vec<u8>`で開始し、後からゼロコピー最適化を検討する段階的アプローチをとる。早期に動作するシステムを構築し、実測データに基づいて最適化を判断することが重要。
- **アプローチ**: `Vec<u8>`または固定長バッファのシンプルな実装
- **理由**: 初期実装ではゼロコピー最適化は不要、標準的なバッファで十分
- **実装方針**:
  - `Serializer::serialize`は`Vec<u8>`を返す
  - Phase 1ではメモリコピーを許容、実装をシンプルに保つ
- **将来の最適化（Phase 4、オプション）**:
  - `heapless::Vec`や固定長バッファの検討
  - ゼロコピー最適化の評価
- **推奨**: Phase 1はシンプルなVec<u8>、将来の最適化は需要次第

#### 🟢 Medium Risk: マニフェストとバージョン互換
- **なぜ必要か**: 異なるバージョンのアプリケーション間でメッセージをやり取りする際、型定義の進化（フィールド追加・削除、型変更）に対応する必要がある。マニフェスト（型識別子）により、受信側が送信側の型バージョンを認識し、適切なデシリアライザを選択できるようにする。これにより、ローリングアップグレードや異なるバージョンのノード間での通信が可能になる。
- **課題**: Pekko/protoactor-goの`SerializationSetup`・manifest進化ロジックのキャッシュ方針をRust所有権モデルへどうマップするか
- **検討事項**:
  - manifest キャッシュの設計
  - 複数バージョン試行ロジックの優先順位制御
  - Rust所有権モデルでの型進化パターン
- **推奨**: Phase 2で基本的なマニフェスト管理、Phase 4で完全なバージョン互換

#### 🟢 Low Risk: Serde非依存の保証
- **なぜ必要か**: cellactor-rsはno_stdファーストの設計であり、組み込み環境やベアメタル環境での使用を想定している。Serdeに依存すると、no_std環境での使用が制限され、バイナリサイズも増大する。フレームワーク非依存を保つことで、ユーザーが自由にシリアライザを選択でき、Serdeはオプショナルな依存として提供する。これにより、最小限のランタイムを必要とする環境でも使用可能になる。
- **課題**: Serdeをリンクしないビルドバリア（feature gating, CIターゲット）の具体案
- **検討事項**:
  - Feature gatingの設計
  - CIでのno_std + Serde未リンクのビルド検証
  - API契約の明示的なドキュメント化
- **推奨**: Phase 1で基本設計、CIターゲット追加

#### 🟢 Low Risk: ActorRef/ActorPath シリアライズ統合
- **なぜ必要か**: リモートノード間でActorの参照をやり取りする際、既存のActorPath実装と整合性を保ちながら、適切な文字列表現に変換する必要がある。リモート通信では完全修飾パス（`pekko://system@host:port/user/actor`）、ローカル通信やPersistenceではローカルパス（`/user/actor`）を使い分ける必要があり、この変換ロジックを統一的に提供することで、コードの重複を避け、一貫性を保証する。
- **課題**: ActorRef/ActorPathシリアライズの文字列化とfallback仕様
- **検討事項**:
  - 既存ActorPath実装との統合方法
  - リモートアドレス未設定時のローカルパス生成
  - Remoting対応時の拡張性
- **推奨**: Phase 2でローカル対応、Phase 3でリモート統合

---

## 5. 設計フェーズへの推奨事項

### 優先調査項目

#### 1. ActorRef文字列化の段階的実装戦略（必須）
- **Phase 1-2**:
  - `serialized_actor_path`ヘルパー関数の実装
  - リモートアドレス未設定時のエラーハンドリングとfallback仕様
- **Phase 3**:
  - std環境でのスレッドローカル実装
  - `with_serialization_scope`スコープAPI
- **将来**:
  - embassy対応時のtask-local検討
  - Tokio/embassy両対応のハンドル設計
- **成果物**: ActorRef文字列化のAPI設計と段階的実装方針

#### 2. ByteBufferSerializerバッファ抽象（重要）
- **Phase 1**: シンプルなバッファAPI（`Vec<u8>`）
- **Phase 3**: ゼロコピー最適化（需要が明確になってから検討）
  - `heapless::Vec`での再利用パターン
  - 固定長バッファAPIの設計
  - std環境での軽量バッファとの互換性
- **成果物**: `BufMut`トレイトまたは抽象型の定義

#### 3. Extension初期化フロー（重要）
- **調査内容**:
  - SerializationSetupの登録タイミング
  - ActorSystem初期化時の依存関係管理（`ActorSystemGeneric::bootstrap`のTODO解消）
  - デフォルトシリアライザの自動登録戦略
- **成果物**: 初期化フローのシーケンス図とコード例

#### 4. マニフェスト管理とバージョン互換（Phase 2以降）
- **調査内容**:
  - manifest キャッシュの設計
  - 複数バージョン試行ロジックの優先順位制御
  - Rust所有権モデルでの型進化パターン
- **成果物**: マニフェスト管理の詳細設計とバージョン互換戦略

### 設計決定事項

#### 1. 実装アプローチ
- **決定**: **Option C (Hybrid) を採用**
- **理由**:
  - 段階的リスク低減が可能
  - Phase 1-2で基本機能を提供し、Transport完全対応は後回し
  - 将来のクレート分離を視野に入れたモジュール境界設計
  - no_std/std環境の差を段階的に吸収

#### 2. モジュール配置
- **決定**: `modules/actor-core/src/serialization/` で実装（Option A採用）
- **理由**: 既存Extension機構との統合が容易、段階的な拡張が可能
- **将来**: 必要に応じて`modules/serialization-core`への分離を検討

#### 3. 段階的実装順序
- **Phase 1 (MVP)**: Serializerトレイト、Registry、Setup (Requirement 1, 4の一部)
  - ActorRef文字列化ヘルパー、組み込みシリアライザ
- **Phase 2 (互換)**: Manifest管理、SerializedMessage (Requirement 2)
  - ActorRef文字列化統合
- **Phase 3 (拡張)**: スレッドローカル最適化（std環境）、AsyncSerializer、ByteBufferSerializer (Requirement 3, 4完全)
- **Phase 4 (完全)**: ネスト委譲API完全対応、バージョン互換ロジック (Requirement 6完全)

#### 4. 技術的制約の明示
- **ActorRef文字列化**: Phase 1-2はExtension内状態管理、Phase 3でstd環境スレッドローカル対応
- **ByteBufferSerializer**: Phase 1はシンプルなバッファ、Phase 3でno_std専用ゼロコピー抽象
- **AsyncSerializer**: Phase 3でFeature flagで分離、std環境のみ対応（初期）
- **Serde非依存**: Phase 1で基本設計、Feature gatingとCIターゲット追加

### 設計フェーズで作成すべきドキュメント

#### 1. API設計書
- 全トレイトのシグネチャとドキュメント（rustdoc英語）
- Builder DSLの使用例（日本語コメント）
- エラーハンドリング戦略とエラー型定義
- 段階的実装における各PhaseのAPI範囲

#### 2. ActorRef文字列化設計書
- Phase 1-2: Extension内状態管理のAPI設計
- Phase 3: std環境でのスレッドローカル対応設計
- ActorPath実装との統合
- エラーパスとフォールバック仕様
- 将来のno_std環境task-local対応の方向性

#### 3. マイグレーションガイド
- Pekkoからの移行パターン
- protoactor-go互換性の維持戦略
- 既存ActorSystemへの統合手順
- 段階的な機能追加による移行パス

#### 4. no_std対応設計書
- Feature gatingの設計
- ByteBufferSerializerのno_std対応戦略
- ActorRef文字列化のno_std/std環境差異
- CIでのno_stdビルド検証方針

---

## 6. 次のステップ

### 1. 設計フェーズへの移行
```bash
/kiro:spec-design serialization-pekko-compatible-redesign
```
- 上記の優先調査項目を実施し、技術的制約を解決
- Option C (Hybrid)の詳細設計を作成
- 各Phaseのマイルストーンと成果物を定義

### 2. デザインレビュー（オプション）
```bash
/kiro:validate-design serialization-pekko-compatible-redesign
```
- Pekko互換性とno_std制約の両立を確認
- 段階的実装計画の妥当性を検証
- ActorRef文字列化の段階的対応戦略をレビュー

### 3. タスク分解
```bash
/kiro:spec-tasks serialization-pekko-compatible-redesign
```
- Phase 1-4の段階的実装計画を具体化
- 各タスクの依存関係と優先順位を明確化
- 技術調査タスクとオーナー・期限を設定

### 4. Phase 1実装開始
- Phase 1 (MVP)のタスクから実装開始
- 各マイルストーンで動作確認とテスト
- CI検証（`./scripts/ci-check.sh all`）を実施

---

## 7. リスク管理計画

### 主要リスク要因と緩和策

#### 🔴 High Risk: マニフェスト/エラーモデル設計（Phase 2）
- **リスク**: 設計を誤ると後続のRemoting/Persistenceに致命的影響
- **緩和策**:
  - Pekko/protoactor-goの実装を詳細に分析
  - Phase 2開始前に設計レビューを実施
  - 単純なマニフェスト管理から開始し、段階的に拡張
- **代替案**: 初期実装は単一バージョンのみサポート、Phase 4で複数バージョン対応を追加
- **検証**: Phase 2でマニフェスト管理の基本動作を確認、互換性テストを実施

#### 🟡 Medium Risk: ByteBufferSerializer設計（Phase 3、オプション）
- **リスク**: no_std環境でのゼロコピー実装が複雑化する可能性
- **緩和策**: Phase 1でシンプルなバッファAPI実装、Phase 3で最適化を検討
- **代替案**: `heapless::Vec`または固定長バッファを標準とし、`bytes`は将来検討
- **検証**: Phase 1でバッファ抽象の基本設計を完成させ、Phase 3で最適化可能性を評価

#### 🟢 Low Risk: ActorRef文字列化ヘルパー（Phase 1-2、必須）
- **アプローチ**: Extension内状態管理で実装（Pekkoにもフォールバックあり）
- **Phase 3拡張（オプション）**: std環境でのスレッドローカル対応を利便性向上として検討
- **検証**: Phase 1でヘルパー関数の基本動作を確認、十分に機能することを検証

### 技術調査のタイムライン

| 調査項目 | 期限 | オーナー | 成果物 |
|---------|------|---------|--------|
| ActorRef文字列化ヘルパーAPI設計 | Phase 1開始前 | 設計担当 | API仕様書 |
| シンプルバッファAPI設計 | Phase 1開始前 | 設計担当 | バッファ抽象仕様 |
| Extension初期化フロー設計 | Phase 1開始前 | 設計担当 | シーケンス図 |
| マニフェスト管理基本設計 | Phase 2開始前 | 設計担当 | manifest管理仕様 |
| ActorRef文字列化統合設計 | Phase 2開始前 | 設計担当 | 統合API仕様 |

---

## 8. ドキュメントステータス

✅ **統合ギャップ分析完了**: Claude分析とCodex分析を統合し、以下を網羅

### Claude分析の貢献
- **詳細な現状調査**: Extension/Messaging/ActorSystem基盤の詳細確認
- **包括的な要求マッピング**: 6つの要求事項と既存実装の詳細対応状況
- **3つの実装アプローチ**: 選択肢とハイブリッド戦略の詳細提案
- **コンポーネント別評価**: 詳細な工数/リスク評価とトレードオフ分析
- **技術的課題の明示**: ActorRef文字列化など3つの主要リスクの詳細分析

### Codex分析の貢献
- **プロジェクト固有制約**: 構造ルール、技術方針、品質基準の明確化
- **インテグレーション視点**: ActorPath/Pid/ActorRefとの統合面の指摘
- **実装規約の強調**: 1ファイル1型、mod.rs禁止、テスト配置などの具体的ルール
- **Labor/Risk評価**: XL工数、High Riskの総合評価
- **Research Needed**: 具体的な調査項目の列挙

### 統合の価値
- **段階的実装戦略**: Phase 1-4の明確な段階分けとActorRef文字列化の段階的対応
- **リスク管理計画**: High Riskコンポーネントの緩和策と技術調査タイムライン
- **プロジェクト適合**: cellactor-rs固有の制約とPekko互換性の両立戦略
- **実装可能性**: 各Phaseで動作確認しながら段階的にリスクを低減

---

## 参考資料

### Pekko実装ファイル
- `/Users/j5ik2o/Sources/cellactor-rs/references/pekko/actor/src/main/scala/org/apache/pekko/serialization/Serializer.scala`
- `/Users/j5ik2o/Sources/cellactor-rs/references/pekko/actor/src/main/scala/org/apache/pekko/serialization/Serialization.scala`
- `/Users/j5ik2o/Sources/cellactor-rs/references/pekko/actor/src/main/scala/org/apache/pekko/serialization/SerializationSetup.scala`

### プロジェクトドキュメント
- `.kiro/specs/serialization-pekko-compatible-redesign/requirements.md`
- `.kiro/steering/structure.md`
- `.kiro/steering/tech.md`
- `AGENTS.md`

### cellactor-rs実装ファイル
- `modules/actor-core/src/system/base.rs` (Extension管理)
- `modules/actor-core/src/messaging/any_message*.rs` (メッセージ表現)
- `modules/utils-core/src/runtime_toolbox/` (Runtime抽象)
