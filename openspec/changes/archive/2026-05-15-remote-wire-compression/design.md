## 背景

`RemoteCompressionConfig` は `actor_ref_max` / `manifest_max` と advertisement interval を保持しているが、現在は wire-level compression へ接続されていない。実コードでは outbound `OutboundEnvelope` から `EnvelopePdu` への変換と actor-core serialization は `TcpRemoteTransport` が担っているため、compression table を `RemoteEvent` に載せて `Remote` 本体へ押し込むと、PDU 変換境界を大きく作り替えることになる。

この change では table 型と wire PDU を `remote-core` に置き、runtime ownership と timer / peer writer 連携は std TCP adaptor に閉じる。`remote-core` は引き続き no_std で、std timer、task、socket、shared channel は `remote-adaptor-std` に残す。

## 目標 / 非目標

**目標:**

- actor ref と serializer manifest の compression table を fraktor-native wire path に適用する。
- table advertisement / acknowledgement / hit counting を実装し、ack 済み entry だけを compressed reference として使う。
- compression disabled 時、未 ack 時、table miss 時は literal encode に戻し、message delivery を止めない。
- inbound compressed reference が未知 id を指す場合は observable decode / transport failure にし、誤配送しない。
- `remote-core` の table state と wire metadata は `core` / `alloc` の範囲で表現する。

**非目標:**

- payload bytes の圧縮。
- Pekko Artery との byte-level compatibility。
- serializer registry / `SerializationExtension` の契約変更。
- remote deployment daemon や cluster membership 連携。
- `RemoteEvent` の新規 variant 追加。

## 設計判断

### 判断 1: table 型は remote-core、所有と timer は std TCP adaptor

`remote-core` に compression table、entry kind、generation、entry id、hit count、advertisement / ack 用 data 型を追加する。`TcpRemoteTransport` は peer ごとにその table 型を保持し、outbound PDU 変換、inbound frame resolution、advertisement timer に使う。

代替案: `Remote` / `Association` が table を所有し、transport は core から compressed metadata を受け取る。これは `RemoteTransport::send(OutboundEnvelope)` の境界と std adaptor の serialization 責務を同時に変更するため、この change の不具合修正範囲に対して churn が大きい。

### 判断 2: compression control は TCP transport が処理して core event loop へ流さない

`ControlPdu::CompressionAdvertisement` と `ControlPdu::CompressionAck` は wire-level metadata 同期であり、actor lifecycle / DeathWatch / flush semantics ではない。TCP reader は compression control を検出したら local table を更新し、必要な ack を peer writer へ返す。`RemoteEvent::InboundFrameReceived` へは、既存の heartbeat / quarantine / shutdown / flush / ack 系 control と、literal に復元済みの envelope のみを流す。

代替案: `RemoteEvent::CompressionAdvertisementTimerFired` と inbound control handling を `Remote` に追加する。これは core event enum と remote loop の責務を増やすが、transport 側で完結できる wire metadata 同期に対して利点が薄い。

### 判断 3: compressed text は literal/reference の明示 enum とする

actor path と manifest は wire 上で `CompressedText` として表現する。`Literal(String)` は従来通り文字列を保持し、`TableRef(u32)` は table id を保持する。recipient / sender は actor-ref table、manifest は manifest table を使う。serializer id と payload bytes は compression 対象にしない。

代替案: `String` の特殊 sentinel や空文字を reference として流用する。これは parse-don't-validate に反し、未知 id と通常文字列の区別を壊すため採用しない。

### 判断 4: ack 済み entry のみ compressed reference を使う

outbound table は observed literals の hit count から generation ごとの advertisement を作る。peer から ack が返るまでは literal encode を続ける。ack 済み generation の entry だけを `TableRef` として使う。

この選択により、table sync の race で message delivery が止まる頻度を抑えられる。inbound に未知 reference が来た場合は peer の protocol violation として失敗させる。

### 判断 5: max = None は kind 単位の local outbound 無効化

`RemoteCompressionConfig::actor_ref_max() == None` の場合 actor-ref の local outbound compression を無効化し、`manifest_max() == None` の場合 manifest の local outbound compression を無効化する。無効化された kind では hit count も advertisement も compressed reference encode も行わない。

この設定は peer から届く inbound advertisement の拒否条件ではない。inbound table は peer の advertisement を保存して ack し、後続 envelope の table reference を復元できなければならない。これにより、一方だけが outbound compression を無効化した接続でも peer の pending generation が永久に詰まらない。

## リスク / トレードオフ

- 既存の frame codec は context-free であるため、TCP transport に compressed envelope metadata を core delivery 前に解決する compression-aware encode / decode wrapper が必要になる。
- Control PDU を transport で消費するため、compression control が actor-level control として転送されないことを tests で確認する必要がある。
- table entry churn によって id が不安定になる可能性があるため、table generation tests で deterministic entry ordering と ack-scoped reuse を検証する。
- compression bug は routing path を壊し得るため、inbound unknown table reference は fail closed にし、誤った recipient へ delivery しない。

## 移行計画

1. `remote-core` に no_std table data 型と compressed wire metadata 型を追加する。
2. `ControlPdu` と envelope wire codec に advertisement / ack / `CompressedText` 表現を追加する。
3. `TcpRemoteTransport` に peer-local compression tables を保持させ、outbound serialization 後に compressed metadata を選択する。
4. TCP reader / framed codec path で inbound compressed references を literal `EnvelopePdu` へ復元し、compression control は transport 内で処理する。
5. `RemoteCompressionConfig` の max / interval を transport construction と timer scheduling に反映する。

## 未解決事項

- 現時点ではなし。wire metadata 型名は spec / tasks と同じく `CompressedText` に固定する。
