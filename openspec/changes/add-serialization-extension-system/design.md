# Serialization Extension System 設計書

## コンテキスト

cellactor-rs は Pekko/Akka を参照した分散アクターランタイムであり、リモートメッセージングを実現するためには「Extension + Serialization」レイヤが必須となる。現状は `no_std`/`alloc` 環境で動作するプラガブルなシリアライザ基盤が存在しないため、Pekko の `SerializationExtension` と同程度の柔軟性・運用互換性を満たす設計を提供する。

## ゴール

1. ActorSystem ごとに Extension を登録できるトレイト/ID ペアの整備
2. Pekko の Serializer SPI (`identifier`/`manifest`/`serialize`/`deserialize`) を Rust/no_std で再現
3. `serializer_id` を主キーにした Rolling Update 互換の `SerializedPayload` を導入
4. 型付き API (`serialize::<T>`, `deserialize::<T>`) を提供しつつ、型登録は事前に行う（送信時の暗黙登録は禁止）
5. すべてのデータ構造を `alloc` のみで構築し、`std` 機能は feature でオプトイン

## アーキテクチャ

### Extension トレイト

```rust
pub trait Extension<TB: RuntimeToolbox>: Send + Sync + 'static {}

pub trait ExtensionId<TB: RuntimeToolbox>: Send + Sync + 'static {
    type Ext: Extension<TB>;
    fn create_extension(&self, system: &ActorSystemGeneric<TB>) -> Self::Ext;
    fn id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}
```

`ActorSystemGeneric` は `register_extension` / `extension` / `has_extension` を提供し、`SystemStateGeneric` 側で `RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>` に格納する。put-if-absent で同一 ExtensionId の多重生成を防ぐ。

### Serializer SPI

Pekko の `Serializer` / `SerializerWithStringManifest` を Rust に移植するため、`SerializerImpl` を次のように定義する。

```rust
pub trait SerializerImpl: Send + Sync {
    fn identifier(&self) -> u32;
    fn serialize_erased(
        &self,
        value: &dyn erased_serde::Serialize,
    ) -> Result<Bytes, SerializationError>;
    fn deserialize(
        &self,
        bytes: &[u8],
        manifest: &str,
    ) -> Result<Box<dyn Any + Send>, SerializationError>;
}
```

- `identifier` はプロセス内で一意であり、Rolling Update 期間も固定。
- `deserialize` で manifest を検証し、不明な場合は `SerializationError::UnknownManifest { serializer_id, manifest }` を返す。

`SerializerHandle` は `Arc<dyn SerializerImpl>` を包む薄いラッパーで、クローンして各レイヤから共有できる。

### SerializerRegistry

データ構造:

```rust
struct SerializerRegistry {
    serializers: RwLock<HashMap<u32, SerializerHandle>>,
    type_bindings: RwLock<HashMap<TypeId, Arc<TypeBinding>>>,
    manifest_bindings: RwLock<HashMap<(u32, String), Arc<TypeBinding>>>,
}

struct TypeBinding {
    type_id: TypeId,
    manifest: String,
    serializer_id: u32,
    serializer: SerializerHandle,
    deserialize_typed: BoxedDeserializer,
}

type BoxedDeserializer = Arc<dyn Fn(&[u8]) -> Result<Box<dyn Any + Send>, SerializationError> + Send + Sync>;
```

- `register_serializer(handle)` は `handle.identifier()` をキーにし、重複時は `SerializationError::DuplicateSerializerId` で失敗。
- `bind_type::<T>` は呼び出し側から manifest を必須で受け取り、未指定の場合はランタイムが `type_name::<T>()` などの規約値を採用する。登録時に `(serializer_id, manifest)` の一意性をチェックして `InvalidManifest` を返す。
- 解除 API を提供し、ローリングアップデートで旧 manifest を外せるようにする。

### Serialization フロー

1. **Serialize**
   1. `TypeId` で TypeBinding を取得。存在しなければ `NoSerializerForType`。
   2. Binding が保持する `serializer` で `serialize_erased` を呼び、`manifest` は Binding の値を利用。
   3. `SerializedPayload { serializer_id, manifest, bytes }` を返す。3 つの値は常にセット。
2. **Deserialize (`T` 指定)**
   1. TypeBinding を再取得し、受け取った manifest と一致するか検証。
   2. `deserialize_typed` クロージャで `T` を復元。失敗時は `DeserializationFailed`。
3. **Deserialize Payload (型未指定)**
   1. `serializer_id` で `SerializerHandle` を取得。存在しない場合は `SerializerNotFound`。
   2. `serializer.deserialize(bytes, manifest)` を実行し `Box<dyn Any + Send>` を返す。
   3. `(serializer_id, manifest)` に対する TypeBinding が存在すれば参照し、一致しない場合は `InvalidManifest`。

### Rolling Update 対応

- `SerializedPayload` は常に `serializer_id` を主キーに処理するため、異なるノードで manifest が同一でも問題ない。
- manifest の構造は `crate::path::Type@v1` のようにバージョン表現を許容し、serializer 側で判定して適切に migrate する。

### BincodeSerializer

- `identifier = 1` を予約し、`serde::Serialize`/`DeserializeOwned` を使った高速な組み込み実装を提供。
- `deserialize` は `bincode::deserialize` の結果を `Box<dyn Any + Send>` に包む。
- `Serialization::new()` で `BincodeSerializer` を登録するが、型バインディングは利用側が明示的に行う。これにより、未登録の型は必ず `NoSerializerForType` となり、暗黙の自動登録を排除できる。

### エラー体系

`SerializationError` には以下を含める。

- `DuplicateSerializerId(u32)`
- `UnknownManifest { serializer_id: u32, manifest: String }`
- `InvalidManifest(String)`
- `SerializerNotFound(u32)`
- `NoSerializerForType(&'static str)`
- `SerializationFailed(String)` / `DeserializationFailed(String)`
- `TypeMismatch { expected: String, found: String }`

`std` feature 有効時のみ `std::error::Error` を実装する。

### no_std / スレッドセーフ

- すべてのコレクションは `alloc` の `Vec`, `BTreeMap`, `Arc`, `RwLock`（`spin`/`parking_lot` の `no_std` 互換版）を使用。
- `Bytes` 型は `alloc::vec::Vec<u8>` のラッパーとして実装する（std の `bytes` crate ではなく）。
- `SerializerHandle` / Registry は `Send + Sync` を満たし、複数スレッドでの登録/検索を `RwLock` によって保護する。

## 決定事項とトレードオフ

1. **送信時の暗黙登録は禁止**: Pekko では設定ファイルで明示的にクラス -> シリアライザを結び付ける。Rust 版でも TypeBinding は起動時に登録し、送信時の自動登録を行わないことで、Rolling Update 時の不一致や偶発的な manifest 漏れを防ぐ。
2. **`serializer_id` を最優先キーにする**: manifest 重複を許容する代わりに `(serializer_id, manifest)` の組を厳格に扱う。旧バージョン (id=10) と新バージョン (id=20) が同じ manifest を共有しても問題なく共存できる。
3. **SerializerImpl に deserialize を含める**: 旧設計では TypeBinding のクロージャで復元していたが、リモートから `Box<dyn Any>` を得るには serializer 自身が責務を持つ必要がある。これにより Pekko の Serializer SPI と一致し、manifest だけで復元経路を切り替えられる。
5. **no_std 応援**: 依存クレートは `erased-serde`, `bincode`, `serde` を `default-features = false` で利用し、`alloc` のみで動作する構成を CI で検証する。

## 未解決事項 / フォローアップ

- Serializer ID の払い出し規約をプロジェクト全体で管理する必要がある。暫定的に 0〜99 を core、100 以降をアプリケーション用とする案を別ドキュメントで定義する。
- manifest 文字列のバージョニング規約（`Type@v1` 等）をどこまでフレームワークで強制するかは今後のオプション。
