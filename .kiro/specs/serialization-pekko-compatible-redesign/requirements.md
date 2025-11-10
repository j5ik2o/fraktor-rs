# Requirements Document / 要求仕様ドキュメント

## Project Description (Input) / プロジェクト概要（入力）
Serialization機能再設計(Pekko互換仕様)

## Introduction / 序文
Pekko と protoactor-go のシリアライゼーション設計を参考に、ActorSystem 全体で一貫した識別子・マニフェスト・Transport コンテキストを扱える仕組みを再確立する。no_std 志向のランタイムでも同一 API を維持しつつ、拡張性と高速経路を両立するための要求を以下に定義する。

## Requirements / 要件

### Requirement 1: シリアライザ識別子とレジストリ統制
**Objective:** ランタイム管理者として、起動時に設定とプログラム登録を統合した衝突のないレジストリを構築し、常に正しいシリアライザを即座に解決したい。

#### Acceptance Criteria
1. When ActorSystem起動時にSerialization Extensionの初期化が開始される, Serialization shall `programmatic setup builder > (optional) external config adapter > built-in default` の順で登録ソースを評価し、当該ライブラリではプログラムAPIのみでレジストリを完成させられるようにする。
2. If 2つ以上のシリアライザが同一の識別子を宣言する, then Serialization shall 初期化を失敗させて衝突内容をログへ記録し、ActorSystemの起動を中止する。
3. When 型情報からシリアライザ解決が要求される, the SerializationRegistry shall TypeId完全一致 > `SerializationSetup` で宣言されたトレイトタグや明示バインディング > 最終フォールバックシリアライザ（例: AnySerializer）の順で探索し、各段階の結果をキャッシュして次回以降をメモリアクセスのみで完了させる。
4. If 該当するシリアライザが見つからない, then Serialization shall NotSerializableエラーを返し、型名と要求元PIDおよび利用中Transportヒントをエラーデータに含める。
5. When シリアライザidentifierが登録される, Serialization shall 0-40番をRuntime予約域として拒否し、ユーザ定義IDの衝突・バージョン情報を監査ログへ記録して Pekko 互換の命名規則を維持する。
6. When アプリケーションが型とシリアライザのバインディングをAPI経由で登録する, Serialization shall 提供するBuilder/DSLで `register_serializer` と `bind::<Marker>()`（構造体・marker trait などのTypeIdを持つ型）を連鎖させ、設定ファイルを使わずに Pekko の `serialization-bindings` と同等の構成を再現できるようにする。

### Requirement 2: マニフェスト互換と型進化
**Objective:** Persistence/Remoting利用者として、マニフェストを使った進化互換と再試行可能なエラー処理を備えた堅牢なシリアライゼーションを求める。

#### Acceptance Criteria
1. When マニフェスト対応シリアライザが同期シリアライズを実行する, Serialization shall 事前にmanifestを取得してバイト列とともにSerializedMessageへ格納する。
2. If 受信側でマニフェスト文字列が未登録または旧バージョンと判断される, then Serialization shall NotSerializableエラー（manifest/serializerId/送信元ヒントを含む）を返し、Transport/Persistence層がリトライや接続維持を判断できる情報を提供する。
3. While 複数バージョンのマニフェスト互換ロジックが設定されている, the SerializationRegistry shall 優先順位リストに従って順次デシリアライズを試行し、成功時点で残りのロジックをスキップする。
4. When シリアライザが「マニフェスト不要」と宣言する, Serialization shall TypeIdベースの直接復元を同一バイナリ内のローカル呼び出しに限定し、Persistence/Remotingへ送る際は論理マニフェスト文字列を付与するショートカットを自動適用しない。

### Requirement 3: ActorRef文字列化とExtension統合
**Objective:** リモート通信開発者として、ActorRefを完全修飾パスまたはローカルパスとして正しくシリアライズ/デシリアライズできるようにしたい。

#### Acceptance Criteria
1. When Serializer実装がActorRefをシリアライズする必要がある場合, the Serialization Extension shall `serialized_actor_path(actor_ref)` ヘルパー関数を提供し、ActorPathを適切な文字列形式に変換する。
   - **重要**: SerializerトレイトはActorRefの内部実装に依存しない（Pekkoと同様: Serializer.scala:65）。
2. When `serialized_actor_path` が呼ばれる, the helper shall リモートアドレスを含む完全修飾パス、またはローカルパスを返す（Pekko互換のフォールバック: Serialization.scala:76-82）。
3. When RemotingやPersistenceがSerializationを呼び出す, the Serialization Extension shall スコープ管理APIを提供し、ActorRefシリアライズに必要なコンテキストを適切に設定・復元する（Pekko: Serialization.scala:114-122）。
4. When Serialization Extensionがシャットダウンを開始する, Serialization shall 関連キャッシュを即時クリアし、以降のアクセスに未初期化エラーを返す。
5. When Serialization Extensionの `serialize` / `deserialize` APIが呼ばれる, Serialization shall 必要なコンテキストを自動設定し、呼び出し完了時に必ず元の状態へ復元する。

### Requirement 4: 組み込みシリアライザラインアップ
**Objective:** ランタイム利用者として、最低限の組み込みシリアライザを備えた柔軟な拡張基盤を求める。

#### Acceptance Criteria
1. When ActorSystemが起動する, the Serialization Extension shall Null/Primitive/String/Bytes向けの組み込みシリアライザを自動登録し、基本的な型のシリアライゼーションをすぐに利用可能にする。
2. When ActorRefシリアライザが実装される, the ActorRefSerializer shall `SerializationExtension::serialized_actor_path(actor_ref)` ヘルパーを呼び出してActorPathを文字列化する（Pekko互換: Serialization.scala:70-93）。
3. When シリアライザがバイト列を生成する, Serializer shall シンプルな`Vec<u8>`バッファを返し、初期実装ではメモリコピーを許容して実装をシンプルに保つ。
   - **Note**: ゼロコピー最適化（`BytesMut`再利用など）は将来の性能改善として検討する。

#### スコープアウト（将来検討）
- **AsyncSerializer**: 非同期シリアライゼーションAPI（初期実装では同期のみで十分）
- **ゼロコピー最適化**: `BytesMut`バッファ再利用によるゼロコピー経路（需要が明確になってから検討）

### Requirement 5: Serde非依存の仕様定義
**Objective:** ランタイム設計者として、Serdeを利用する実装を許容しつつも仕様レイヤーが特定フレームワークへ依存しないことを保証したい。

#### Acceptance Criteria
1. When シリアライゼーションAPIを文書化する, Serialization shall Rust Serde固有の型・トレイト・マクロを仕様レベルの契約に含めない。
2. If 個別実装がSerde専用シリアライザを提供したい, then the SerializationRegistry shall それらを任意の追加エントリとして登録できるようにし、既定レジストリへ自動追加しない。
3. While no_stdやSerde未対応ターゲットでActorSystemが動作している, Serialization shall Serde依存コードをリンクせずに全コア機能を利用可能であることを保証する。
4. When 拡張ガイドやテンプレートがシリアライザ追加手順を示す, Serialization shall Serde以外の実装にも適用できる抽象API記述で共通化する。

### Requirement 6: ネスト型の再帰委譲
**Objective:** カスタムシリアライザ作者として、複合メッセージ内の個々のフィールドを既存シリアライザへ委譲し、仕様全体で一貫したフォーマットとエラー管理を維持したい。

#### Acceptance Criteria
1. When A型のシリアライザが内部フィールドBのシリアライズを必要とする, Serialization shall 公開APIを通じて登録済みBシリアライザへ委譲できるようにする。
2. If Bフィールドに対応するシリアライザが未登録である, then Serialization shall Result型でNotSerializableエラーを返し、Aシリアライザがエラーを伝播するか明示ログへ転写する運用ガイドを提供する。
3. While 再帰的に委譲されたシリアル化が実行されている, Serialization shall Manifest情報を親呼び出しと共有し、ネストしたシリアライズが正しく動作するよう維持する。ActorRefシリアライズに必要なコンテキストはスコープ管理により自動的に共有される。
4. When デシリアライズで委譲ルートを辿る, Serialization shall Bのデシリアライザを呼び出した結果をそのままAの復元に利用できるようにし、成功/失敗をAシリアライザへ伝播する。

## Appendix / 付録

### コアトレイト案
Pekko互換のシリアライザトレイト群の最小仕様。

```rust
/// 基本シリアライザトレイト（Pekko Serializer互換）
pub trait Serializer: Send + Sync {
    /// シリアライザ識別子を返す（Pekkoのidentifier）
    fn identifier(&self) -> SerializerId;

    /// メッセージをバイト列にシリアライズ（PekkoのtoBinary）
    fn to_binary(&self, msg: &dyn Any) -> Result<Vec<u8>, SerializationError>;

    /// バイト列からメッセージをデシリアライズ（PekkoのfromBinary）
    /// type_hintはPekkoのmanifest: Option[Class[_]]に相当
    fn from_binary(
        &self,
        bytes: &[u8],
        type_hint: Option<TypeId>,
    ) -> Result<Box<dyn Any + Send>, SerializationError>;

    /// マニフェストを含めるか（PekkoのincludeManifest）
    fn include_manifest(&self) -> bool {
        false
    }

    /// ダウンキャスト用のAny参照
    fn as_any(&self) -> &dyn Any;
}

/// 文字列マニフェスト対応シリアライザ（Pekko SerializerWithStringManifest互換）
pub trait SerializerWithStringManifest: Serializer {
    /// メッセージの型マニフェストを取得（PekkoのSerializerWithStringManifest.manifest）
    /// Pekkoではエラーなしで必ず成功するためStringを返す
    fn manifest(&self, msg: &dyn Any) -> String;

    /// マニフェスト付きデシリアライズ（Pekkoのfrominary(bytes, manifest: String)）
    fn from_binary_with_manifest(
        &self,
        bytes: &[u8],
        manifest: &str,
    ) -> Result<Box<dyn Any + Send>, SerializationError>;
}

/// ゼロコピー最適化シリアライザ（Pekko ByteBufferSerializer互換）
/// デシリアライズはSerializerのfrom_binaryを使用
pub trait ByteBufferSerializer: Serializer {
    /// バッファ再利用でシリアライズ（PekkoのtoBinary(o, buf)）
    fn to_buffer(&self, msg: &dyn Any, buf: &mut impl BufMut) -> Result<(), SerializationError>;
}
```

### SerializedMessage構造体
Pekkoのシリアライズ結果を表現する中核データ構造。

```rust
/// シリアライズ結果（Pekko SerializedMessage互換）
#[derive(Debug, Clone)]
pub struct SerializedMessage {
    /// シリアライザ識別子
    pub serializer_id: SerializerId,
    /// 型マニフェスト（オプション）
    pub manifest: Option<String>,
    /// シリアライズされたバイト列（Phase 1: シンプルな Vec<u8>）
    pub bytes: Vec<u8>,
}

impl SerializedMessage {
    /// Pekko互換フォーマットにエンコード
    /// フォーマット: [serializer_id: u32][has_manifest: u8][manifest_len: u32]?[manifest]?[payload_len: u32][payload]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Serializer ID
        buf.extend_from_slice(&self.serializer_id.0.to_le_bytes());

        // Manifest
        if let Some(manifest) = &self.manifest {
            buf.push(1); // has manifest flag
            let manifest_bytes = manifest.as_bytes();
            buf.extend_from_slice(&(manifest_bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(manifest_bytes);
        } else {
            buf.push(0); // no manifest
        }

        // Payload
        buf.extend_from_slice(&(self.bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.bytes);

        buf
    }

    /// Pekko互換フォーマットからデコード
    pub fn decode(bytes: &[u8]) -> Result<Self, SerializationError> {
        if bytes.len() < 5 {
            return Err(SerializationError::InvalidFormat);
        }

        let mut offset = 0;
        let serializer_id = SerializerId::new(u32::from_le_bytes([
            bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]
        ]));
        offset += 4;

        let has_manifest = bytes[offset] != 0;
        offset += 1;

        let manifest = if has_manifest {
            if bytes.len() < offset + 4 {
                return Err(SerializationError::InvalidFormat);
            }
            let len = u32::from_le_bytes([
                bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]
            ]) as usize;
            offset += 4;

            if bytes.len() < offset + len {
                return Err(SerializationError::InvalidFormat);
            }
            let manifest_str = String::from_utf8(bytes[offset..offset+len].to_vec())?;
            offset += len;
            Some(manifest_str)
        } else {
            None
        };

        if bytes.len() < offset + 4 {
            return Err(SerializationError::InvalidFormat);
        }
        let payload_len = u32::from_le_bytes([
            bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]
        ]) as usize;
        offset += 4;

        if bytes.len() < offset + payload_len {
            return Err(SerializationError::InvalidFormat);
        }
        let payload = bytes[offset..offset+payload_len].to_vec();

        Ok(SerializedMessage {
            serializer_id,
            manifest,
            bytes: payload,
        })
    }
}
```

### Serialization API
メッセージのシリアライズ/デシリアライズを統括するサービスAPI。

```rust
pub struct Serialization {
    registry: Arc<SerializationRegistry>,
}

impl Serialization {
    /// メッセージをシリアライズ（Pekko Serialization.serialize互換）
    pub fn serialize(&self, msg: &dyn Any) -> Result<SerializedMessage, SerializationError> {
        let type_id = msg.type_id();
        let serializer = self.registry.serializer_for_type(type_id)?;

        let bytes = serializer.to_binary(msg)?;

        let manifest = if serializer.include_manifest() {
            // SerializerWithStringManifestの場合はmanifestを取得
            if let Some(sm) = serializer.as_any().downcast_ref::<dyn SerializerWithStringManifest>() {
                Some(sm.manifest(msg))
            } else {
                None
            }
        } else {
            None
        };

        Ok(SerializedMessage {
            serializer_id: serializer.identifier(),
            manifest,
            bytes,
        })
    }

    /// SerializedMessageからデシリアライズ（Pekko Serialization.deserialize互換）
    pub fn deserialize(
        &self,
        serialized: &SerializedMessage,
    ) -> Result<Box<dyn Any + Send>, SerializationError> {
        let serializer = self.registry.serializer_by_id(serialized.serializer_id)?;

        if let Some(manifest) = &serialized.manifest {
            // SerializerWithStringManifestの場合
            if let Some(sm) = serializer.as_any().downcast_ref::<dyn SerializerWithStringManifest>() {
                return sm.from_binary_with_manifest(&serialized.bytes, manifest);
            }
        }

        // 通常のSerializerの場合
        serializer.from_binary(&serialized.bytes, None)
    }

    /// 型ヒント付きデシリアライズ
    pub fn deserialize_with_type_hint(
        &self,
        serialized: &SerializedMessage,
        type_hint: TypeId,
    ) -> Result<Box<dyn Any + Send>, SerializationError> {
        let serializer = self.registry.serializer_by_id(serialized.serializer_id)?;

        if let Some(manifest) = &serialized.manifest {
            if let Some(sm) = serializer.as_any().downcast_ref::<dyn SerializerWithStringManifest>() {
                return sm.from_binary_with_manifest(&serialized.bytes, manifest);
            }
        }

        serializer.from_binary(&serialized.bytes, Some(type_hint))
    }
}
```

### SerializerバインディングAPI例
設定ファイルを使わずに型とシリアライザの対応を構築するBuilder DSLの一例。
```rust
let registry = SerializationBuilder::new()
    .register_serializer("serde-json", SerializerId::new(101), SerdeJsonSerializer::new())
    .register_serializer("serde-cbor", SerializerId::new(102), SerdeCborSerializer::new())
    .register_serializer("prost", SerializerId::new(103), ProtobufSerializer::new())
    .register_serializer("custom", SerializerId::new(201), MyOwnSerializer::new())
    .bind::<JsonSerializable>("serde-json")
    .bind::<CborSerializable>("serde-cbor")
    .bind::<prost::Message>("prost")
    .bind::<MyOwnSerializable>("custom")
    .build()?;
```
ここで `JsonSerializable` / `CborSerializable` / `MyOwnSerializable` は零サイズの marker trait または newtype で、TypeId を媒介にシリアライザ解決を行う。

### ネスト委譲メソッド例
カスタムシリアライザ`ASerializer`内で`Serialization`のAPIを呼び出し、Bフィールドを委譲する際の実装イメージ。

```rust
/// A型のカスタムシリアライザ（ネスト型B, nameフィールドを持つ）
pub struct ASerializer {
    id: SerializerId,
    serialization: Arc<Serialization>,
}

impl Serializer for ASerializer {
    fn identifier(&self) -> SerializerId {
        self.id
    }

    fn to_binary(&self, msg: &dyn Any) -> Result<Vec<u8>, SerializationError> {
        let a = msg
            .downcast_ref::<A>()
            .ok_or(SerializationError::TypeMismatch)?;

        let mut buf = Vec::new();

        // Bフィールドをシリアライズ（SerializedMessage全体を取得）
        let b_serialized = self.serialization.serialize(&a.b)?;
        let b_encoded = b_serialized.encode(); // Pekko互換フォーマットにエンコード
        buf.extend_from_slice(&b_encoded);

        // nameフィールドをシリアライズ
        let name_serialized = self.serialization.serialize(&a.name)?;
        let name_encoded = name_serialized.encode();
        buf.extend_from_slice(&name_encoded);

        Ok(buf)
    }

    fn from_binary(
        &self,
        bytes: &[u8],
        _type_hint: Option<TypeId>,
    ) -> Result<Box<dyn Any + Send>, SerializationError> {
        let mut offset = 0;

        // Bフィールドをデシリアライズ
        let b_serialized = SerializedMessage::decode(&bytes[offset..])?;
        let b_encoded_len = b_serialized.encode().len();
        offset += b_encoded_len;

        let b_any = self.serialization.deserialize(&b_serialized)?;
        let b = *b_any
            .downcast::<B>()
            .map_err(|_| SerializationError::TypeMismatch)?;

        // nameフィールドをデシリアライズ
        let name_serialized = SerializedMessage::decode(&bytes[offset..])?;
        let name_any = self.serialization.deserialize(&name_serialized)?;
        let name = *name_any
            .downcast::<String>()
            .map_err(|_| SerializationError::TypeMismatch)?;

        Ok(Box::new(A { name, b }))
    }

    fn include_manifest(&self) -> bool {
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// SerializerWithStringManifestを使う場合の例
impl SerializerWithStringManifest for ASerializer {
    fn manifest(&self, _msg: &dyn Any) -> String {
        "example.A".to_string()
    }

    fn from_binary_with_manifest(
        &self,
        bytes: &[u8],
        manifest: &str,
    ) -> Result<Box<dyn Any + Send>, SerializationError> {
        // マニフェストでバージョン管理などを行う
        match manifest {
            "example.A" | "example.A.v1" => self.from_binary(bytes, None),
            _ => Err(SerializationError::UnknownManifest(manifest.to_string())),
        }
    }
}
```
