# remote-adaptor-std-tcp-transport Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: TcpRemoteTransport 型

`fraktor_remote_adaptor_std_rs::std::tcp_transport::TcpRemoteTransport` 型が定義され、core の `RemoteTransport` trait を実装する SHALL。Pekko `ArteryTcpTransport` に対応する TCP ベースの remote transport を提供する。

#### Scenario: 型の存在

- **WHEN** `modules/remote-adaptor-std/src/tcp_transport/tcp_transport.rs` を読む
- **THEN** `pub struct TcpRemoteTransport` が定義されている

#### Scenario: RemoteTransport trait の実装

- **WHEN** `TcpRemoteTransport` の trait 実装を検査する
- **THEN** `impl RemoteTransport for TcpRemoteTransport` が存在し、core の全メソッド (`start`, `shutdown`, `send`, `addresses`, `default_address`, `local_address_for_remote`, `quarantine`) を実装している

### Requirement: bind と accept loop

`TcpRemoteTransport::start` は `tokio::net::TcpListener::bind` でリスナーを開始し、accept loop の tokio task を spawn する SHALL。

#### Scenario: bind 成功後に accept loop が動作

- **WHEN** `TcpRemoteTransport::new(config)` で作成し `start()` を呼ぶ
- **THEN** 指定されたアドレスで `TcpListener` が bind され、accept loop の tokio task が生成される

#### Scenario: bind 失敗時のエラー

- **WHEN** 既に使用中のポートに対して `start()` を呼ぶ
- **THEN** `Err(TransportError::SendFailed)` または同等のエラーが返る

### Requirement: connect とハンドシェイク

リモートエンドポイントへの送信時、接続がまだ確立されていなければ `TcpStream::connect` で接続を開始し、handshake を開始する SHALL。

#### Scenario: 初回送信時の connect

- **WHEN** まだ接続していないリモートアドレスに対して `send(envelope)` を呼ぶ
- **THEN** `TcpStream::connect` が内部で実行され、handshake が開始される

#### Scenario: handshake 完了前の envelope 蓄積

- **WHEN** handshake 完了前に複数の `send(envelope)` を呼ぶ
- **THEN** envelope は core の `Association` の deferred queue に蓄積され、handshake 完了後にまとめて送信される

### Requirement: Framed codec 統合

`tcp_transport::frame_codec` モジュールは `tokio_util::codec::{Encoder, Decoder}` を実装し、core の `Codec<T>` trait と tokio の Framed streaming を統合する SHALL。

#### Scenario: Framed の利用

- **WHEN** `tcp_transport::connection` 系モジュールで `TcpStream` を Framed 化する箇所を検査する
- **THEN** `tokio_util::codec::Framed` が使われており、core の wire frame header (length+version+kind) を正しく解釈する

#### Scenario: core Codec との整合

- **WHEN** adapter の Framed decoder が受信した bytes を decode する
- **THEN** 内部で core の `Codec<T>::decode` が呼ばれ、PDU に変換される

### Requirement: `Instant::now()` の呼び出し場所の局所化

`Instant::now()` の呼び出しは adapter 側の特定箇所 (主に `handshake_driver`・`heartbeat` タイマー発火時) のみに限定される SHALL。core には `Instant::now()` を渡さず、常に monotonic millis の `u64` に変換してから core API を呼ぶ。

#### Scenario: Instant::now() の使用箇所

- **WHEN** `modules/remote-adaptor-std/src/` 配下を `Instant::now()` で grep する
- **THEN** 使用箇所は限定され、すべて monotonic millis への変換を伴う (wall clock `SystemTime::now()` は使わない)

