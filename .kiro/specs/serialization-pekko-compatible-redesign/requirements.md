# Requirements Document / 要求仕様ドキュメント

## Project Description (Input) / プロジェクト概要（入力）
Serialization機能再設計(Pekko互換仕様)

## Introduction / 序文
Pekko と protoactor-go のシリアライゼーション設計を参考に、ActorSystem 全体で一貫した識別子・マニフェスト・Transport コンテキストを扱える仕組みを再確立する。no_std 志向のランタイムでも同一 API を維持しつつ、拡張性と高速経路を両立するための要求を以下に定義する。

## Requirements / 要件

### Requirement 1: シリアライザ識別子とレジストリ統制
**Objective:** ランタイム管理者として、起動時に設定とプログラム登録を統合した衝突のないレジストリを構築し、常に正しいシリアライザを即座に解決したい。

#### Acceptance Criteria
1. When ActorSystem起動時にSerialization Extensionの初期化が開始される, the Serializationサービス shall 設定値とコード登録を優先順位規則に従ってマージし、完了時に一意の識別子を持つレジストリを構築する。
2. If 2つ以上のシリアライザが同一の識別子を宣言する, then the Serializationサービス shall 初期化を失敗させて衝突内容をログへ記録し、ActorSystemの起動を中止する。
3. When 型情報からシリアライザ解決が要求される, the Serializationレジストリ shall TypeId完全一致 > トレイトタグや明示的に登録された代替候補 > フォールバックシリアライザの順で探索し、各段階の結果をキャッシュして次回以降をメモリアクセスのみで完了させる。
4. If 該当するシリアライザが見つからない, then the Serializationサービス shall NotSerializableエラーを返し、型名と要求元PIDおよび利用中Transportヒントをエラーデータに含める。

### Requirement 2: マニフェスト互換と型進化
**Objective:** Persistence/Remoting利用者として、マニフェストを使った進化互換と再試行可能なエラー処理を備えた堅牢なシリアライゼーションを求める。

#### Acceptance Criteria
1. When マニフェスト対応シリアライザが同期シリアライズを実行する, the Serializationサービス shall 事前にmanifestを取得してバイト列とともにSerializedMessageへ格納する。
2. If 受信側でマニフェスト文字列が未登録または旧バージョンと判断される, then the Serializationサービス shall NotSerializableエラー（manifest/serializerId/送信元ヒントを含む）を返し、Transport/Persistence層がリトライや接続維持を判断できる情報を提供する。
3. While 複数バージョンのマニフェスト互換ロジックが設定されている, the Serializationレジストリ shall 優先順位リストに従って順次デシリアライズを試行し、成功時点で残りのロジックをスキップする。
4. When シリアライザが「マニフェスト不要」と宣言する, the Serializationサービス shall TypeIdベースの直接復元を同一バイナリ内のローカル呼び出しに限定し、Persistence/Remotingへ送る際は論理マニフェスト文字列を付与するショートカットを自動適用しない。

### Requirement 3: TransportコンテキストとExtension統合
**Objective:** リモート通信開発者として、Transport情報とExtensionスコープが常に整合し、ActorRef文字列化やコンテキスト依存シリアライズが破綻しないようにしたい。

#### Acceptance Criteria
1. When RemotingやPersistenceがSerializationを呼び出す, the Serialization Extension shall Transport情報を (a) シリアライザAPI引数での明示受け渡し もしくは (b) thread-local などTokio task localに依存しないスレッドスコープなハンドル を通じて提供し、どちらを使うかを実装ガイドで明示する。
2. If シリアライザがTransport情報へアクセスした際にスコープ外である, then the Serialization Extension shall エラーを返し、呼び出し元へTransport設定の欠落を通知する。
3. While Transport情報が設定されている, the Serializationサービス shall ActorRefやActorPathをシリアライズするときに該当アドレス情報を含めて書き出す。
4. When Serialization Extensionがシャットダウンを開始する, the Serializationサービス shall Transportスコープと関連キャッシュを即時クリアし、以降のアクセスに未初期化エラーを返す。

### Requirement 4: 非同期・高速パスと組み込みラインアップ
**Objective:** ランタイム利用者として、非同期処理・ゼロコピー経路・最低限の組み込みシリアライザを備えた柔軟な拡張基盤を求める。

#### Acceptance Criteria
1. When AsyncSerializerが登録されている, the Serializationサービス shall 非同期APIを優先実行してFuture/Promiseを返し、呼び出し側がノンブロッキングで結果を待機できるようにする。
2. If 同期APIがAsyncSerializerを内部で呼び出す, then the Serializationサービス shall 警告ログを出力し、同期待機が呼び出し側責務であることを明示した上で結果を返す。
3. While BytesSerializerやRawBytes高速パスが有効になっている, the Serializationサービス shall `BytesSerializer`トレイト（`fn to_bytes_mut(&self, msg: &dyn Any, buf: &mut BytesMut)`）のように `&mut BytesMut` を受け取るAPIでバッファ再利用を定義し、ゼロコピーを仕様として明示する。
4. When ActorSystemが起動する, the Serialization Extension shall Null/Primitive/String/Bytes向けの組み込みシリアライザのみを登録し、ActorRefは Transport Extension 提供の専用シリアライザへ委譲する。
5. If ActorRefのシリアライズ要求にTransport情報が添付されていない, then the Serializationサービス shall 明示的なエラーを返し、Transport Extensionを経由したAPIを利用するようガイドする。

### Requirement 5: Serde非依存の仕様定義
**Objective:** ランタイム設計者として、Serdeを利用する実装を許容しつつも仕様レイヤーが特定フレームワークへ依存しないことを保証したい。

#### Acceptance Criteria
1. When シリアライゼーションAPIを文書化する, the Serializationサービス shall Rust Serde固有の型・トレイト・マクロを仕様レベルの契約に含めない。
2. If 個別実装がSerde専用シリアライザを提供したい, then the Serializationレジストリ shall それらを任意の追加エントリとして登録できるようにし、既定レジストリへ自動追加しない。
3. While no_stdやSerde未対応ターゲットでActorSystemが動作している, the Serializationサービス shall Serde依存コードをリンクせずに全コア機能を利用可能であることを保証する。
4. When 拡張ガイドやテンプレートがシリアライザ追加手順を示す, the Serializationサービス shall Serde以外の実装にも適用できる抽象API記述で共通化する。

### Requirement 6: ネスト型の再帰委譲
**Objective:** カスタムシリアライザ作者として、複合メッセージ内の個々のフィールドを既存シリアライザへ委譲し、仕様全体で一貫したフォーマットとエラー管理を維持したい。

#### Acceptance Criteria
1. When A型のシリアライザが内部フィールドBのシリアライズを必要とする, the Serializationサービス shall 公開APIを通じて登録済みBシリアライザへ委譲できるようにする。
2. If Bフィールドに対応するシリアライザが未登録である, then the Serializationサービス shall Result型でNotSerializableエラーを返し、Aシリアライザがエラーを伝播するか明示ログへ転写する運用ガイドを提供する。
3. While 再帰的に委譲されたシリアル化が実行されている, the Serializationサービス shall Manifest/Transport情報を親呼び出しと共有し、ActorRefやアドレス情報がダブルシリアライズされないよう維持する。
4. When デシリアライズで委譲ルートを辿る, the Serializationサービス shall Bのデシリアライザを呼び出した結果をそのままAの復元に利用できるようにし、成功/失敗をAシリアライザへ伝播する。

## Appendix / 付録

### BytesSerializerインタフェース例
ゼロコピー経路をRustの所有権に沿って表現するための具体的なトレイト像。
```rust
pub trait BytesSerializer {
    fn to_bytes_mut(&self, msg: &dyn Any, buf: &mut BytesMut) -> Result<(), SerializationError>;
    fn from_bytes(&self, buf: &mut Bytes) -> Result<Box<dyn Any + Send>, SerializationError>;
}
```

### AからBへの委譲コード例
カスタムシリアライザAが登録済みのBシリアライザへ委譲する際の呼び出し順序を示す。
```rust
fn serialize_a(a: &A, svc: &SerializationService) -> Result<Bytes, SerializationError> {
    let mut buf = BytesMut::new();
    svc.serialize_field::<B>(&a.b, &mut buf)?;
    svc.serialize_field::<String>(&a.name, &mut buf)?;
    Ok(buf.freeze())
}
```
