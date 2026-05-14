## ADDED Requirements

### Requirement: association flush session state

`Association` は shutdown flush と DeathWatch notification 前 flush の session state を所有する SHALL。session state は flush id、flush scope、caller が渡した対象 writer lane id 集合、期待 ack 数、ack 済み lane id 集合、deadline monotonic millis、完了状態を保持しなければならない（MUST）。timer、async wait、TCP の lane topology 推定は保持してはならない（MUST NOT）。

#### Scenario: shutdown flush starts session for caller supplied lanes

- **GIVEN** caller が shutdown flush の対象 writer lane id として `[0, 1, 2]` を渡す
- **WHEN** active association に shutdown flush を開始する
- **THEN** association は新しい flush id を割り当てる
- **AND** lane `0`、`1`、`2` を対象にした flush request effect を返す
- **AND** expected ack 数は `3` になる
- **AND** deadline は caller から渡された monotonic millis と flush timeout から計算される

#### Scenario: DeathWatch flush uses caller supplied message-capable lanes

- **GIVEN** caller が DeathWatch notification 前 flush の対象 writer lane id として `[0, 1]` を渡す
- **WHEN** active association に DeathWatch notification 前 flush を開始する
- **THEN** association は lane `0`、`1` を対象にした flush request effect を返す
- **AND** association は `lane_id = 0` を control-only lane と仮定して除外しない

#### Scenario: empty lane set completes immediately

- **WHEN** caller が空の対象 writer lane id 集合で flush を開始する
- **THEN** association は flush request effect を返さない
- **AND** flush completed effect を即時に返す

#### Scenario: flush ack completes session

- **GIVEN** flush id `10` の session が lane `0` と lane `1` の ack を待っている
- **WHEN** lane `0` と lane `1` の `FlushAck` を association に適用する
- **THEN** association は flush completed effect を返す
- **AND** session は active flush map から削除される

#### Scenario: duplicate flush ack is ignored

- **GIVEN** flush id `10` の session が lane `0` の ack をすでに観測している
- **WHEN** lane `0` の `FlushAck` を再度 association に適用する
- **THEN** remaining ack count は減らない
- **AND** duplicate ack だけでは flush completed effect を返さない

#### Scenario: flush timeout releases session

- **GIVEN** flush id `10` の session が lane `1` の ack を待っている
- **WHEN** monotonic millis が session deadline 以上になった timer input を association に適用する
- **THEN** association は flush timed-out effect を返す
- **AND** session は active flush map から削除される

#### Scenario: connection loss fails pending flush

- **GIVEN** active association に pending flush session がある
- **WHEN** connection lost または quarantine transition が association に適用される
- **THEN** association は pending flush session を failed または timed-out outcome として完了させる effect を返す
- **AND** caller が shutdown または DeathWatch notification の後続処理へ進める

#### Scenario: flush does not start while prior outbound queue is still pending

- **GIVEN** flush 開始より前に association outbound queue に未送信 envelope が残っている
- **WHEN** caller が flush session を開始しようとする
- **THEN** association または `Remote` の flush start path は flush request effect を返さない
- **AND** flush start failure または timeout outcome を観測可能にする
- **AND** flush completed を ordering guarantee として返してはならない

### Requirement: flush effects are transport-neutral

`AssociationEffect` は flush request の送信、flush completed、flush timed out、flush failed を transport-neutral な effect として表現する SHALL。effect は concrete TCP handle、tokio task、`JoinHandle`、channel sender を含んではならない（MUST NOT）。

#### Scenario: start flush returns send effects

- **WHEN** association が flush session を開始する
- **THEN** effect は remote authority、flush id、flush scope、対象 writer lane id、期待 ack 数を含む
- **AND** std adaptor はこの effect から `ControlPdu::FlushRequest` を作れる

#### Scenario: completed effect identifies original flush

- **WHEN** association が flush completed effect を返す
- **THEN** effect は flush id と flush scope を含む
- **AND** std adaptor は shutdown flush と DeathWatch notification 前 flush を区別できる

#### Scenario: timed out effect identifies remaining lanes

- **WHEN** association が flush timed-out effect を返す
- **THEN** effect は flush id、flush scope、ack 未到達の lane id を含む
- **AND** timeout は log または test-observable path に渡せる
