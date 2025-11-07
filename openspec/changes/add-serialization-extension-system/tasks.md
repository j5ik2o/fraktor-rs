# Serialization Extension System 実装タスク

## タスク依存関係

- 1.x（基盤実装）を完了してから 2.x（Extension 本体）と 3.x（組み込みシリアライザ）に進む
- 2.1.2 の API 実装は 1.3/1.4 の Serializer/Registry 完了を前提とする
- 3.1（BincodeSerializer）は 1.3/1.4/2.1.2 の最低限 API が揃ってから着手する
- 5.x の各テストは対応する実装セクション（1.x/2.x/3.x/4.x）完了後に実行し、CI (7.x) は最後にまとめて行う
- no_std ビルド検証（7.4）は 3.1 まで完了し、`serializer_id` ベースの API が `alloc` のみで動作することを確認してから行う

## 1. 基盤実装

### 1.1 Extension トレイトシステムの実装
- [ ] 1.1.1 `modules/actor-core/src/extension.rs` を作成
  - `Extension<TB>` トレイトの定義
  - `ExtensionId<TB>` トレイトの定義
  - RustDoc コメントの追加

- [ ] 1.1.2 `modules/actor-core/src/lib.rs` に Extension モジュールを追加
  - `pub mod extension;` を追加
  - 公開APIとして `Extension` と `ExtensionId` を re-export

- [ ] 1.1.3 `ActorSystemGeneric` に Extension 管理機能を追加
  - `modules/actor-core/src/system/base.rs` を編集
  - `register_extension<E>(&self, ext_id: &E) -> Arc<E::Ext>` メソッドを追加
  - `extension<E>(&self, ext_id: &E) -> Option<Arc<E::Ext>>` メソッドを追加
  - `has_extension<E>(&self, ext_id: &E) -> bool` メソッドを追加

- [ ] 1.1.4 `SystemStateGeneric` に Extension ストレージを追加
  - `modules/actor-core/src/system/system_state.rs` を編集
  - `extensions: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>` フィールドを追加
  - Extension 管理用の内部メソッドを追加

### 1.2 Serialization エラー型の実装
- [ ] 1.2.1 `modules/actor-core/src/serialization/error.rs` を更新
  - 既存の `SerializationError` を Pekko 互換に拡張
  - `Display` トレイトの実装
  - `std::error::Error` トレイト実装（feature "std" のみ）

### 1.3 Serializer API の実装
- [ ] 1.3.1 `modules/actor-core/src/serialization/serializer.rs` を作成
  - `SerializerImpl` トレイトに Pekko 互換メソッドを定義
    - `fn identifier(&self) -> u32`
    - `fn serialize_erased(&self, value: &dyn erased_serde::Serialize) -> Result<Bytes, SerializationError>`
    - `fn deserialize(&self, bytes: &[u8], manifest: &str) -> Result<Box<dyn Any + Send>, SerializationError>`
  - RustDoc は英語で記載し、`erased-serde` 依存と Pekko との互換ポリシーを明示
  - オブジェクト安全性と `no_std` 対応 (`alloc` のみ) を満たす
- [ ] 1.3.2 `SerializerHandle` 構造体を実装
  - `Arc<dyn SerializerImpl>` を保持し、Clone でハンドルを複製可能にする
  - `identifier()`, `serialize_erased()`, `deserialize()` を透過的に委譲
  - `Send + Sync` を保証し、Extension から安全に共有できるようにする

### 1.4 SerializerRegistry の実装
- [ ] 1.4.1 `modules/actor-core/src/serialization/registry.rs` を作成
  - `type_bindings`（TypeId→TypeBinding）、`manifest_bindings`（(serializer_id, manifest)→TypeBinding）、`serializers`（id→SerializerHandle）の3テーブルを `RwLock` で保持
  - TypeBinding には `type_id`, `manifest`, `serializer_id`, `serializer: SerializerHandle`, `deserialize_typed: BoxedDeserializer` を含める
  - `BoxedDeserializer` は `Arc<dyn Fn(&[u8]) -> Result<Box<dyn Any + Send>, SerializationError> + Send + Sync>` 型で定義
- [ ] 1.4.2 `register_serializer()` の仕様を実装
  - 追加しようとする ID が既に存在する場合は `SerializationError::DuplicateSerializerId` を返して拒否
  - `SerializerHandle::identifier()` と整合することを検証
- [ ] 1.4.3 `bind_type()` API を実装
  - `bind_type::<T: Serialize + DeserializeOwned + Send + 'static>(serializer: SerializerHandle, manifest: Option<String>, deserializer: impl Fn(&[u8]) -> Result<T, SerializationError> + Send + Sync + 'static)` を提供
  - manifest 未指定時は `core::any::type_name::<T>()` を使用し、`(serializer.identifier(), manifest)` の一意性をチェック
  - デシリアライザクロージャを `BoxedDeserializer` に型消去して TypeBinding を作成し、`type_bindings` と `manifest_bindings` の両方に登録
- [ ] 1.4.4 フェッチ API の拡充
  - `find_binding_by_type::<T>()` は `Result<Arc<TypeBinding>, SerializationError>` を返す（未登録なら `NoSerializerForType`）
  - `find_binding_by_manifest(serializer_id, manifest)` は `(serializer_id, manifest)` で検索し、見つからなければ `InvalidManifest` を返す
  - `find_serializer_by_id(u32)` は `SerializerHandle` を返し、存在しなければ `SerializerNotFound`
- [ ] 1.4.5 解除 API
  - `unbind_type(TypeId)` および `(serializer_id, manifest)` 単位の解除 API を用意し、ローリングアップデート時に旧 binding を破棄できるようにする

## 2. Serialization Extension の実装

### 2.1 SerializationExtension の実装
- [ ] 2.1.1 `modules/actor-core/src/serialization/extension.rs` を作成
  - `SerializationExtensionId` 構造体の定義
  - `SERIALIZATION_EXTENSION` グローバル定数の定義
  - `ExtensionId<TB>` トレイトの実装
  - `Serialization<TB>` 構造体の定義
  - `Extension<TB>` トレイトの実装

- [ ] 2.1.2 `Serialization<TB>` に API メソッドを実装
  - `serialize<T: Serialize + 'static>(&self, obj: &T) -> Result<SerializedPayload, SerializationError>`
    - `find_binding_by_type::<T>()` で TypeBinding を取得できなければ即 `NoSerializerForType`
    - Binding に保存された `serializer_id` と manifest を使用し、`SerializerHandle::serialize_erased()` でバイト列を生成
    - `SerializedPayload { serializer_id, manifest, bytes }` を返し、3 要素すべてを埋める
  - `deserialize<T: DeserializeOwned + 'static>(&self, bytes: &[u8], manifest: &str) -> Result<T, SerializationError>`
    - 登録済み TypeBinding を `find_binding_by_type::<T>()` で取得し、manifest が一致しない場合は `TypeMismatch`
    - Binding の `deserialize_typed` クロージャを実行し `T` を返す
  - `find_serializer_for<T: 'static>(&self) -> Result<SerializerHandle, SerializationError>`
    - TypeBinding から `serializer` を複製して返す
  - `registry(&self) -> Arc<SerializerRegistry>`
    - 内部の SerializerRegistry を公開（カスタム登録/解除用）
  - `deserialize_payload(&self, payload: &SerializedPayload) -> Result<Box<dyn Any + Send>, SerializationError>`
    - `payload.serializer_id` から `SerializerHandle` を取得できなければ `SerializerNotFound`
    - 取得したシリアライザで `deserialize(bytes, manifest)` を実行し、`Box<dyn Any + Send>` を復元
    - `(serializer_id, manifest)` に対応する TypeBinding がある場合は `manifest_bindings` を用いて型ヒントを照合し、不整合なら `InvalidManifest`

- [ ] 2.1.3 `SerializedPayload` 構造体を追加
  - `serializer_id`, `manifest`, `bytes` を保持し、リモート伝送に必要な情報をカプセル化
  - `serde::Serialize` / `Deserialize` を実装し、将来的なリモート機能に備える

### 2.2 ActorSystem への統合
- [ ] 2.2.1 `ActorSystemGeneric::new()` で SerializationExtension を自動登録
  - `modules/actor-core/src/system/base.rs` の初期化処理を編集
  - システム起動時に `SERIALIZATION_EXTENSION` を登録

## 3. Bincode シリアライザの実装

### 3.1 BincodeSerializer の実装
- [ ] 3.1.1 `modules/actor-core/src/serialization/bincode_serializer.rs` を作成
  - `BincodeSerializer` 構造体の定義
  - `SerializerImpl` トレイトの実装
    - `identifier()` は固定値（例: `1`）を返す
    - `serialize_erased()` は `erased_serde::serialize` + `bincode::DefaultOptions` で実装
    - `deserialize()` は `bincode::deserialize` に委譲し、`Box<dyn Any + Send>` を返す

- [ ] 3.1.2 `Cargo.toml` に bincode 依存を追加
  - `bincode = { version = "1.3", default-features = false, features = ["alloc"] }`
  - `erased-serde = { version = "0.3", default-features = false }` を同時に追加し、`no_std` でも利用できることを確認

- [ ] 3.1.3 フレームワークの組み込みシリアライザとして登録
  - `Serialization::new()` 内で以下を実行：
    1. `BincodeSerializer` のインスタンスを作成
    2. `SerializerHandle` でラップ
    3. `registry.register_serializer()` でシリアライザ ID を登録
  - 型ごとの `bind_type` は利用側で行う前提とし、自動バインディングは行わない

## 4. モジュール構成の整理

### 4.1 serialization モジュールの mod.rs 作成
- [ ] 4.1.1 `modules/actor-core/src/serialization/mod.rs` を作成
  - 各サブモジュールを宣言
  - 公開APIを re-export

### 4.2 lib.rs への追加
- [ ] 4.2.1 `modules/actor-core/src/lib.rs` を編集
  - `pub mod serialization;` を追加（既に存在する場合はスキップ）

## 5. テストの実装

### 5.1 Extension システムのテスト
- [ ] 5.1.1 `modules/actor-core/src/extension/tests.rs` を作成
  - Extension の登録・取得のテスト
  - 同じ ExtensionId で複数回登録した場合の動作テスト（同じインスタンスを返す）
  - 異なる ExtensionId の独立性テスト

### 5.2 Serialization Extension のテスト
- [ ] 5.2.1 `modules/actor-core/src/serialization/extension/tests.rs` を作成
  - `Serialization::serialize()` / `deserialize()` のラウンドトリップテスト
  - カスタムメッセージ型のシリアライゼーションテスト
  - エラーケースのテスト（未登録型、不正なバイト列等）
  - `(serializer_id, manifest)` が一致しない場合に `InvalidManifest` が返ることを検証
  - 未登録型は常に `NoSerializerForType` になることを検証
  - `deserialize_payload()` が `serializer_id`/manifest の組で復元できることを検証
  - `serializer_id` に対応するシリアライザが存在しない場合は `SerializerNotFound`
  - manifest が登録済みと異なる場合は `InvalidManifest` もしくは `UnknownManifest` を返すことを検証

### 5.3 BincodeSerializer のテスト
- [ ] 5.3.1 `modules/actor-core/src/serialization/bincode_serializer/tests.rs` を作成
  - 基本型（String, i32, Vec等）のシリアライゼーションテスト
  - 構造体のシリアライゼーションテスト
  - エラーケースのテスト
  - `SerializerHandle` 経由で `Serialize` トレイトオブジェクトが `serialize_erased` に到達することを検証

### 5.4 SerializerRegistry のテスト
- [ ] 5.4.1 `modules/actor-core/src/serialization/registry/tests.rs` を作成
  - シリアライザの登録・取得テスト
  - 型バインディングのテスト
    - `bind_type::<T>()` でクロージャが正しく保存されることを検証
    - 保存されたデシリアライザクロージャが `Box<dyn Any>` を返すことを検証
  - manifest バインディングの登録・検索テスト
    - 同じ `(serializer_id, manifest)` の重複登録が `InvalidManifest` エラーになることを検証
    - manifest 文字列のフォーマット検証（空文字列、不正文字等）
  - スレッドセーフのテスト（複数スレッドからの同時アクセス）
  - TypeBinding の `deserialize_boxed` を呼び出して型復元できることを検証
    - クロージャが正しく型情報をキャプチャしていることを確認

## 6. ドキュメントの追加

### 6.1 RustDoc の充実
- [ ] 6.1.1 すべての公開API に `///` コメントを追加
- [ ] 6.1.2 使用例を `/// # Examples` セクションに追加
- [ ] 6.1.3 `# Panics`, `# Errors` セクションを必要に応じて追加

### 6.2 モジュールレベルのドキュメント
- [ ] 6.2.1 `modules/actor-core/src/extension.rs` にモジュールドキュメントを追加
- [ ] 6.2.2 `modules/actor-core/src/serialization/mod.rs` にモジュールドキュメントを追加

## 7. CI / Lint の対応

### 7.1 Clippy / Dylint 対応
- [ ] 7.1.1 `./scripts/ci-check.sh all` を実行し、すべてのチェックをパス
- [ ] 7.1.2 必要に応じて `#[allow(...)]` を追加（理由をコメントで記載）

### 7.2 フォーマット
- [ ] 7.2.1 `cargo +nightly fmt` を実行

### 7.3 テスト実行
- [ ] 7.3.1 `cargo test --all-features` を実行し、すべてのテストがパス

### 7.4 no_std ビルド検証
- [ ] 7.4.1 `cargo build --target thumbv7em-none-eabihf --no-default-features` を実行し、`alloc` のみで serialization モジュールがリンクできることを確認
- [ ] 7.4.2 同ターゲットで `SerializerImpl::deserialize` がすべて `alloc` のみで動作することを確認するための smoke テストを追加

## 8. オプショナル機能（将来対応）

### 8.1 JSON シリアライザの実装
- [ ] 8.1.1 `modules/actor-core/src/serialization/json_serializer.rs` を作成
- [ ] 8.1.2 `Cargo.toml` に `serde-json-core` 依存を追加（feature フラグ付き）
- [ ] 8.1.3 テストを追加

### 8.2 カスタムシリアライザの登録API
- [ ] 8.2.1 `Serialization::register_custom_serializer()` メソッドを追加
- [ ] 8.2.2 ユーザー向けのドキュメントを追加

### 8.3 パフォーマンスベンチマーク
- [ ] 8.3.1 `benches/serialization.rs` を作成
- [ ] 8.3.2 bincode vs JSON のベンチマークを実装
- [ ] 8.3.3 様々なメッセージサイズでの測定

### 8.4 Message derive スパイク
- [ ] 8.4.1 `examples/derive_message` で `#[derive(Message)]` マクロの PoC を作成し、`SERIALIZER_ID`/manifest をコンパイル時に付与できるか検証
- [ ] 8.4.2 Protoactor-go 方式との API 差異を比較し、Extension パターンと併存可能か技術調査ノートを作成
- [ ] 8.4.3 TypeId を使わない経路のベンチマークを 8.3 シナリオに追加し、採用可否の判断材料を揃える

## 完了条件 (Definition of Done)

- [ ] すべてのタスク（1〜7）が完了
- [ ] `./scripts/ci-check.sh all` がパス
- [ ] すべてのテストがパス
- [ ] RustDoc が完備されている
- [ ] コードレビューで承認済み
