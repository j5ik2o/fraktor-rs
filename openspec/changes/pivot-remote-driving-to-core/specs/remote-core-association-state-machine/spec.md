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
- **THEN** 同一呼出または直後の `Remote::run` 経路で `RemoteInstrument::on_send(&envelope)` が呼ばれる
- **AND** 呼び出し点は Association 内部または `Remote::run` の outbound 駆動経路のいずれかで明文化される

#### Scenario: inbound dispatch で on_receive 発火

- **WHEN** `Remote::run` が `Codec::decode` した `InboundEnvelope` を Association に渡す
- **THEN** `RemoteInstrument::on_receive(&envelope)` が呼ばれる

#### Scenario: apply_backpressure で record_backpressure

- **WHEN** `Association::apply_backpressure(signal)` が呼ばれる
- **THEN** `RemoteInstrument::record_backpressure(authority, signal, correlation_id, now_ms)` が呼ばれる
- **AND** correlation_id は backpressure 文脈で観測可能な情報がある場合に限り `Some(_)` を取り、無ければ `None`

### Requirement: instrument 引数の渡し方

`Association` の状態遷移メソッドおよび送受信メソッドは `&mut dyn RemoteInstrument` を引数で受け取り、`Association` 自身が instrument を field として所有してはならない（MUST NOT）。型パラメータ `<I: RemoteInstrument>` を `Association` メソッドに導入してはならない（MUST NOT）。

#### Scenario: instrument を field 保持しない

- **WHEN** `Association` 構造体のフィールドを検査する
- **THEN** `RemoteInstrument` を直接または間接的に保持しない
- **AND** instrument 参照は呼び出し時に外部（`Remote::run`）から渡される

#### Scenario: hook 系メソッドの引数

- **WHEN** `Association::associate` / `handshake_accepted` / `handshake_timed_out` / `quarantine` / `apply_backpressure` の最終シグネチャを検査する
- **THEN** いずれも `instrument: &mut dyn RemoteInstrument` を引数として受け取る経路が確立されている
- **AND** 呼び出し側（`Remote::run`）は `&mut *self.instrument`（`self.instrument: Box<dyn RemoteInstrument + Send>` から `DerefMut` 経由）で参照を取得する
- **AND** メソッドシグネチャに型パラメータ `<I>` が出現しない

### Requirement: outbound queue の総長クエリ

`Association` は outbound queue（`SendQueue` の system + user）の合計長を返すクエリメソッドを提供する SHALL。これは `Remote::run` が watermark backpressure を制御するために使用する。

#### Scenario: total_outbound_len のシグネチャ

- **WHEN** `Association::total_outbound_len` または同等の query method の定義を読む
- **THEN** `fn total_outbound_len(&self) -> usize` が宣言されている（CQS 準拠で `&self`）
- **AND** 戻り値は system priority queue と user priority queue の合計長を表す

#### Scenario: deferred queue は含めない

- **WHEN** `Handshaking` 状態で deferred queue に envelope が積まれている
- **THEN** `total_outbound_len()` は `SendQueue` のみの長さを返し、deferred queue を含めない（deferred は handshake 完了で flush される一時バッファであり、watermark 判定の対象外）

### Requirement: watermark 連動の自動 backpressure 発火経路

`Remote::run` は `Association::total_outbound_len()` を `outbound_high_watermark` / `outbound_low_watermark` と比較し、状態遷移時に `Association::apply_backpressure` を呼び出して signal を発火する SHALL。Association 側は手動 `apply_backpressure` 呼び出しと watermark 経由の自動呼び出しを区別しない（同じ signal セマンティクスで動作する）。

#### Scenario: 既存 BackpressureSignal::Apply / Release の流用

- **WHEN** `BackpressureSignal` enum の variant を検査する
- **THEN** 既存の `Apply` と `Release` のみが定義され、`Engaged` / `Released` 等の新 variant が追加されていない
- **AND** watermark 連動の発火と adapter / 上位層からの手動発火は同じ variant を使う

#### Scenario: backpressure state は Association が保持する

- **WHEN** 同じ signal を 2 回連続で発火する
- **THEN** `Association` は idempotent に動作し、2 回目は state 遷移を伴わない
- **AND** instrument の `record_backpressure` は state 変化を伴った発火点でのみ呼ばれる（または instrument 側で重複を吸収する）

### Requirement: AssociationEffect::StartHandshake は Remote::run で実行される（adapter 無視を禁止）

`Association::recover` および `associate` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を出力した場合、その effect は `Remote::run` の経路で `RemoteTransport` 経由の handshake 開始に dispatch されなければならない（MUST）。adapter 側で `StartHandshake` を ignore する分岐を持ってはならない（MUST NOT）。

#### Scenario: Remote::run による StartHandshake 実行（2 ステップ）

- **WHEN** `Association::recover(Some(endpoint), now)` または `associate(...)` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を返す
- **THEN** `Remote::run` は同一 effect 列処理の中で次の 2 ステップを順に実行する
  1. `Codec::encode` で handshake request envelope を bytes 化し、既存 `RemoteTransport::send` で送出する
  2. 続けて `RemoteTransport::schedule_handshake_timeout(&authority, timeout, generation)`（`remote-core-transport-port` capability で要件化）を呼ぶ
- **AND** ステップ 1 が `Err` の場合、ステップ 2 は呼ばれない
- **AND** adapter 側は `schedule_handshake_timeout` 呼出を契機に tokio task で sleep を起動し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を adapter 内部 sender 経由で source に push する

#### Scenario: adapter 側の StartHandshake 無視分岐の不在

- **WHEN** `modules/remote-adaptor-std/src/std/effect_application.rs` の dispatch を検査する
- **THEN** `AssociationEffect::StartHandshake { .. } => /* ignore */` または同等の no-op 分岐が存在しない

### Requirement: handshake generation の管理（u64 inline）

`Association` は handshake ごとに単調増加する generation 値を `u64` フィールドとして保持し、`AssociationEffect::StartHandshake` および `RemoteEvent::HandshakeTimerFired` で同じ `u64` を参照することで、古い timeout の発火を無視する SHALL。`HandshakeGeneration` 等の newtype は新設してはならない（MUST NOT、純増ゼロ方針）。

#### Scenario: generation の保持

- **WHEN** `Association` 構造体のフィールドを検査する
- **THEN** `handshake_generation: u64` が保持され、`Handshaking` 状態に入るたびに `wrapping_add(1)` で +1 される
- **AND** `HandshakeGeneration` newtype や `pub struct HandshakeGeneration(u64)` が定義されていない

#### Scenario: 古い timeout の無視

- **WHEN** `Remote::run` が `RemoteEvent::HandshakeTimerFired { authority, generation: g_event }` を受信し、現在の `Association` の generation が `g_current` であって `g_current != g_event` である
- **THEN** `Remote::run` は `Association::handshake_timed_out` を呼ばず、event を破棄する
- **AND** 破棄は instrument の `record_handshake` を発火しない（古いイベントなので観測対象外）
- **AND** 比較演算子は `!=` を使用する（`>` は使用しない。`wrapping_add` で +1 を続けると `u64::MAX → 0` の wrap 時に `g_current > g_event` が成立せず stale 判定が漏れるため）

#### Scenario: AssociationEffect::StartHandshake の generation フィールド

- **WHEN** `AssociationEffect::StartHandshake` の variant 定義を検査する
- **THEN** `StartHandshake { authority: TransportEndpoint, timeout: core::time::Duration, generation: u64 }` または同等のフィールド構成を持つ
- **AND** generation の型は `u64` であり、newtype でラップされていない
