## ADDED Requirements

### Requirement: Association は instrument hook を呼び出す

`Association` は状態遷移および送受信のキー点で `RemoteInstrument` の対応 method を呼び出さなければならない（MUST）。instrument 参照は引数として受け取る（field として保持しない）。

#### Scenario: associate / handshake_accepted で record_handshake 発火

- **WHEN** `Association::associate` または `handshake_accepted` が呼ばれる
- **THEN** 同一呼出の中で対応する `RemoteInstrument::record_handshake(authority, phase, now_ms)` が呼ばれる
- **AND** phase は `Started` / `Accepted` / `Rejected` のいずれかを取る

#### Scenario: handshake_timed_out で record_handshake(Rejected)

- **WHEN** `Association::handshake_timed_out` が呼ばれる
- **THEN** `RemoteInstrument::record_handshake(authority, HandshakePhase::Rejected, now_ms)` が呼ばれる

#### Scenario: quarantine で record_quarantine

- **WHEN** `Association::quarantine(reason, now_ms)` が呼ばれる
- **THEN** `RemoteInstrument::record_quarantine(authority, reason, now_ms)` が呼ばれる

#### Scenario: enqueue / next_outbound で on_send 発火

- **WHEN** `Association::next_outbound` が `Some(envelope)` を返す
- **THEN** 同一呼出または直後の Driver 経路で `RemoteInstrument::on_send(&envelope)` が呼ばれる
- **AND** 呼び出し点は Association 内部または Driver の outbound 駆動経路のいずれかで明文化される

#### Scenario: inbound dispatch で on_receive 発火

- **WHEN** Driver が `Codec::decode` した `InboundEnvelope` を Association に渡す
- **THEN** `RemoteInstrument::on_receive(&envelope)` が呼ばれる

#### Scenario: apply_backpressure で record_backpressure

- **WHEN** `Association::apply_backpressure(signal)` が呼ばれる
- **THEN** `RemoteInstrument::record_backpressure(authority, signal, correlation_id, now_ms)` が呼ばれる
- **AND** correlation_id は backpressure 文脈で観測可能な情報がある場合に限り `Some(_)` を取り、無ければ `None`

### Requirement: instrument 引数の渡し方

`Association` の状態遷移メソッドおよび送受信メソッドは `&mut I: RemoteInstrument` または同等の参照を引数で受け取り、`Association` 自身が instrument を field として所有してはならない（MUST NOT）。

#### Scenario: instrument を field 保持しない

- **WHEN** `Association` 構造体のフィールドを検査する
- **THEN** `RemoteInstrument` を直接または間接的に保持しない
- **AND** instrument 参照は呼び出し時に外部（`Remote` または Driver）から渡される

#### Scenario: hook 系メソッドの引数

- **WHEN** `Association::associate` / `handshake_accepted` / `handshake_timed_out` / `quarantine` / `apply_backpressure` の最終シグネチャを検査する
- **THEN** いずれも `instrument: &mut I` または `&I` を引数として受け取る経路が確立されている
  - 直接引数として受け取る、または
  - `Association` を保持する `Remote` のジェネリクス経路から `&mut self` 経由で渡される
- **AND** 呼び出し側（Driver / Remote）から見て instrument 参照が一貫した型 `I` で渡されることが保証される

### Requirement: outbound queue の総長クエリ

`Association` は outbound queue（`SendQueue` の system + user）の合計長を返すクエリメソッドを提供する SHALL。これは Driver が watermark backpressure を制御するために使用する。

#### Scenario: total_outbound_len のシグネチャ

- **WHEN** `Association::total_outbound_len` または同等の query method の定義を読む
- **THEN** `fn total_outbound_len(&self) -> usize` が宣言されている（CQS 準拠で `&self`）
- **AND** 戻り値は system priority queue と user priority queue の合計長を表す

#### Scenario: deferred queue は含めない

- **WHEN** `Handshaking` 状態で deferred queue に envelope が積まれている
- **THEN** `total_outbound_len()` は `SendQueue` のみの長さを返し、deferred queue を含めない（deferred は handshake 完了で flush される一時バッファであり、watermark 判定の対象外）

### Requirement: watermark 連動の自動 backpressure 発火経路

Driver は `Association::total_outbound_len()` を `outbound_high_watermark` / `outbound_low_watermark` と比較し、状態遷移時に `Association::apply_backpressure` を呼び出して signal を発火する SHALL。Association 側は手動 `apply_backpressure` 呼び出しと watermark 経由の自動呼び出しを区別しない（同じ signal セマンティクスで動作する）。

#### Scenario: 既存 BackpressureSignal の意味の整合

- **WHEN** `BackpressureSignal` enum の variant を検査する
- **THEN** `Engaged`（または `Apply`）と `Released`（または `Release`）が定義されている
- **AND** Driver から発火された signal と adapter から発火された signal は同じ effect を生む

#### Scenario: backpressure state は Association が保持する

- **WHEN** 同じ signal を 2 回連続で発火する
- **THEN** `Association` は idempotent に動作し、2 回目は state 遷移を伴わない
- **AND** instrument の `record_backpressure` は state 変化を伴った発火点でのみ呼ばれる（または instrument 側で重複を吸収する）

### Requirement: AssociationEffect::StartHandshake は Driver で実行される（adapter 無視を禁止）

`Association::recover` および `associate` が `AssociationEffect::StartHandshake { endpoint }` を出力した場合、その effect は Driver の経路で `RemoteTransport::initiate_handshake(&endpoint)` に dispatch されなければならない（MUST）。adapter 側で `StartHandshake` を ignore する分岐を持ってはならない（MUST NOT）。

#### Scenario: Driver による StartHandshake 実行

- **WHEN** `Association::recover(Some(endpoint), now)` が `AssociationEffect::StartHandshake` を返す
- **THEN** Driver は同一 effect 列処理の中で `RemoteTransport::initiate_handshake(&endpoint)` を呼ぶ
- **AND** Driver は `Timer::schedule(handshake_timeout, RemoteEvent::HandshakeTimerFired { authority, generation })` で timeout を予約する

#### Scenario: adapter 側の StartHandshake 無視分岐の不在

- **WHEN** `modules/remote-adaptor-std/src/std/effect_application.rs` の dispatch を検査する
- **THEN** `AssociationEffect::StartHandshake { .. } => /* ignore */` または同等の no-op 分岐が存在しない

### Requirement: handshake generation の管理

`Association` は handshake ごとに単調増加する generation 値を保持し、`AssociationEffect::StartHandshake` および `RemoteEvent::HandshakeTimerFired` で同じ generation を参照することで、古い timeout の発火を無視する SHALL。

#### Scenario: generation の保持

- **WHEN** `Association` 構造体のフィールドを検査する
- **THEN** `handshake_generation: HandshakeGeneration`（または `u64` 単純型）が保持され、`Handshaking` 状態に入るたびに +1 される

#### Scenario: 古い timeout の無視

- **WHEN** Driver が `RemoteEvent::HandshakeTimerFired { authority, generation: g_old }` を受信し、現在の `Association` の generation が `g_new > g_old` である
- **THEN** Driver は `Association::handshake_timed_out` を呼ばず、event を破棄する
- **AND** 破棄は instrument の `record_handshake` を発火しない（古いイベントなので観測対象外）
