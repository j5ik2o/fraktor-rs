## MODIFIED Requirements

### Requirement: watermark 連動の自動 backpressure 発火経路

`Remote::handle_remote_event` は outbound enqueue / dequeue のたびに `Association::total_outbound_len()` を watermark と比較し、境界をエッジで跨いだ時にのみ `Association::apply_backpressure` を呼び出す SHALL。high watermark の signal は、internal drain を止めない意味と一致していなければならない（MUST）。

#### Scenario: BackpressureSignal variant の仕様は実装と一致する

- **WHEN** `BackpressureSignal` enum の variant を検査する
- **THEN** live OpenSpec は実装に存在する variant だけを仕様化する
- **AND** `Notify` variant が存在する場合、その意味は「internal high watermark を跨いだことの通知であり、user lane pause はしない」として明記されている
- **AND** `Apply` variant は「user lane を pause する」意味として残る

#### Scenario: Notify は user lane を pause しない

- **GIVEN** `BackpressureSignal::Notify` が high watermark 用に採用されている
- **WHEN** `Association::apply_backpressure(Notify, ...)` を呼ぶ
- **THEN** `SendQueue` の user lane は paused にならない
- **AND** instrumentation には high watermark crossing が記録される

#### Scenario: Apply は明示的な pause を表す

- **WHEN** adapter / upper layer が明示的な backpressure として `BackpressureSignal::Apply` を呼ぶ
- **THEN** user lane は paused になる
- **AND** `BackpressureSignal::Release` で resume する
