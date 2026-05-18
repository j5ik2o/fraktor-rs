## MODIFIED Requirements

### Requirement: メッセージ送信メソッド

`RemoteTransport` trait は `send` メソッドを持ち、`OutboundEnvelope` 単位で送信を要求する SHALL。失敗時は caller が再 enqueue できるよう、元の `OutboundEnvelope` を error と一緒に返さなければならない（MUST）。

#### Scenario: send メソッドは retry 可能な signature を持つ

- **WHEN** `RemoteTransport::send` の定義を読む
- **THEN** `fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), (TransportError, Box<OutboundEnvelope>)>` または同等の元 envelope を返すシグネチャが宣言されている
- **AND** live OpenSpec は `Result<(), TransportError>` だけを要求してはならない

#### Scenario: byte 単位ではなく envelope 単位

- **WHEN** `send` メソッドの引数型を確認する
- **THEN** 引数は `&[u8]` や `Bytes` ではなく `OutboundEnvelope` である
- **AND** wire bytes への変換は transport adapter 実装の責務であり、core port の引数型を raw bytes に変更しない

#### Scenario: error path は ownership を保持する

- **WHEN** transport が running でない、peer connection がない、または payload serialization に失敗する
- **THEN** `send` は error と元 envelope を返す
- **AND** caller は clone なしで元 envelope を retry queue に戻せる
