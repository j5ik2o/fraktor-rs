# Serialization Extension Specification

## ADDED Requirements

### Requirement: Extension System
ActorSystem は ExtensionId ごとに単一インスタンスの Extension を生成・保持しなければならない (SHALL)。Extension は `Send + Sync + 'static` を満たし、再登録時には同一インスタンスが返されなければならない (SHALL)。

#### Scenario: Extension の登録と取得
- **WHEN** ユーザーが `system.register_extension(&MY_EXTENSION)` を呼ぶ
- **THEN** Extension が生成され `Arc<MyExtension>` が返る
- **AND** 同じ ExtensionId で再度呼び出した場合も同じ `Arc` が返る

#### Scenario: 複数 Extension の独立管理
- **WHEN** ユーザーが異なる ExtensionId を登録する
- **THEN** それぞれ独立したストレージ領域に保持され、互いに影響しない

#### Scenario: Extension の存在確認
- **WHEN** `system.has_extension(&MY_EXTENSION)` を呼ぶ
- **THEN** 登録済みなら `true`、未登録なら `false` を返す

### Requirement: Serialization Extension
ActorSystem は起動フックで `SerializationExtension` を自動登録しなければならない (SHALL)。Extension は Pekko の `SerializationExtension` と同じ責務を持ち、シリアライザ識別子・manifest・バイト列を常にセットで扱わなければならない (SHALL)。

#### Scenario: 自動登録
- **WHEN** ActorSystem が起動する
- **THEN** `SerializationExtension` が初期化され、`system.extension(&SERIALIZATION_EXTENSION)` で取得できる

#### Scenario: 送信時の 3 要素保持
- **WHEN** `serialization.serialize(&message)` を呼ぶ
- **THEN** 返却される `SerializedPayload` は `serializer_id`, `manifest`, `bytes` の 3 要素を必ず含む
- **AND** 3 要素のいずれかを欠いた転送を許可してはならない (MUST NOT)

### Requirement: Serializer SPI
カスタムシリアライザは `SerializerImpl` トレイトを実装し、以下を満たさなければならない (SHALL)。
1. `identifier() -> u32` はプロセス全体で一意で固定
2. `serialize(value: &dyn erased_serde::Serialize) -> Result<Bytes, SerializationError>`
3. `deserialize(bytes: &[u8], manifest: &str) -> Result<Box<dyn Any + Send>, SerializationError>`

#### Scenario: ID の一意性
- **WHEN** 2 つのシリアライザが同じ ID を返す
- **THEN** ActorSystem 起動は拒否され、`SerializationError::DuplicateSerializerId` が報告される

#### Scenario: deserialize 呼び出し
- **WHEN** `SerializerImpl::deserialize(bytes, manifest)` が呼ばれる
- **THEN** manifest をヒントに `Box<dyn Any + Send>` を復元する
- **AND** 未対応 manifest の場合は `SerializationError::UnknownManifest { serializer_id, manifest }` を返す

### Requirement: Type Binding API
`TypeBinding` は型とシリアライザの対応関係を記述し、型付き API (`serialize<T>()` / `deserialize<T>()`) 用のメタデータを保持しなければならない (SHALL)。型バインディングの登録は起動時や Extension 初期化時に行い、送信時に自動生成してはならない (MUST NOT)。

#### Scenario: 型バインディング登録
- **WHEN** `registry.bind_type::<MyMessage>(serializer_handle, manifest, typed_deserializer)` を呼ぶ
- **THEN** `MyMessage` の `TypeId` と manifest がストレージに保存され、`typed_deserializer` は後続の `deserialize::<MyMessage>` で再利用される
- **AND** manifest は呼び出し側が安定した完全修飾名を明示的に指定する（省略時は `core::any::type_name::<MyMessage>()` などフレームワーク既定の規約に従う）

#### Scenario: 双方向互換性
- **WHEN** 異なるノードが同じ manifest/serializer_id の組を共有する
- **THEN** 事前に登録された型バインディングにより、どちらのノードでも `MyMessage` を復元できる

### Requirement: SerializerRegistry
`SerializerRegistry` は 3 種のマップを保持する (SHALL)：`type_bindings (TypeId→TypeBinding)`, `manifest_bindings ((serializer_id, manifest)→TypeBinding)`, `serializers (id→SerializerHandle)`。

#### Scenario: シリアライザ登録
- **WHEN** `register_serializer(handle)` を呼ぶ
- **THEN** ID 重複を検知した場合は登録を拒否し `DuplicateSerializerId` を返す

#### Scenario: manifest バインディング
- **WHEN** `registry.bind_type::<MyMessage>(serializer_handle, Some("my_app::MyMessage"), deserializer)` を呼ぶ
- **THEN** `(serializer_handle.identifier(), "my_app::MyMessage")` の組が登録され、同じ組での再登録は `SerializationError::InvalidManifest` となる
- **AND** 旧バージョンのシリアライザに切り替える際は `unbind_type` 等で明示的に削除する

#### Scenario: Payload 復元
- **WHEN** `deserialize_payload` が呼ばれる
- **THEN** まず `serializer_id` から `SerializerHandle` を取得し、次に `(serializer_id, manifest)` で TypeBinding を解決し、`SerializerHandle::deserialize` を実行する
- **AND** どちらかが欠けていれば `SerializationError::SerializerNotFound` または `SerializationError::InvalidManifest` を返す

### Requirement: Explicit Type Registration
すべてのメッセージ型は `bind_type()` などの明示手続きで登録しなければならず、送信時の暗黙登録やデフォルトフォールバックは禁止される (SHALL NOT)。

#### Scenario: 未登録型
- **WHEN** 型バインディングが存在しない型をシリアライズする
- **THEN** 直ちに `SerializationError::NoSerializerForType` が返され、暗黙の登録やデフォルトフォールバックは行われない

### Requirement: Rolling Update 互換性
`SerializedPayload` は Pekko と同様に `serializer_id` を主キーとして復元経路を選択しなければならない (SHALL)。manifest は同じ serializer_id 内でのみ一意であればよく、別 ID で再利用できる (MAY)。

#### Scenario: 新旧シリアライザの共存
- **WHEN** ノード A が `serializer_id = 10`、ノード B が `serializer_id = 20` で同じ manifest を取り扱う
- **THEN** A→B の通信では payload の `serializer_id=10` に基づき旧シリアライザが選択され、manifest はヒントとして渡されるだけで衝突しない

### Requirement: no_std とスレッドセーフ
すべてのデータ構造は `alloc` のみで動作し (SHALL)、`RwLock` 等でマルチスレッド安全性を確保しなければならない (SHALL)。`std` 機能は feature `std` 有効時のみ `std::error::Error` 実装を追加する。

#### Scenario: no_std ビルド
- **WHEN** `#![no_std]` ターゲットでビルドする
- **THEN** Serialization Extension のコンパイルが成功する

#### Scenario: 競合アクセス
- **WHEN** 複数スレッドが同時に `register_serializer` と `serialize` を実行する
- **THEN** データ競合なしに完了し、デッドロックも発生しない
