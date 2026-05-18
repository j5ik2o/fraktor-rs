## ADDED Requirements

### Requirement: shutdown flush timeout is the flush deadline source

`RemoteConfig::shutdown_flush_timeout` は shutdown flush と DeathWatch notification 前 flush の deadline source として使われる SHALL。core state machine には timeout 値そのもの、または timeout から計算された monotonic deadline を caller が渡し、core は wall clock を参照してはならない（MUST NOT）。

#### Scenario: default timeout is used by flush drivers

- **WHEN** caller が `RemoteConfig::new(...)` を使い、flush timeout を明示設定しない
- **THEN** shutdown flush driver と DeathWatch 前 flush driver は default `shutdown_flush_timeout` を使う

#### Scenario: configured timeout is used by both flush paths

- **GIVEN** `RemoteConfig::new(...).with_shutdown_flush_timeout(Duration::from_secs(10))`
- **WHEN** shutdown flush または DeathWatch notification 前 flush を開始する
- **THEN** flush session deadline は 10 秒の timeout を基準に計算される

#### Scenario: zero timeout does not wait forever

- **GIVEN** `shutdown_flush_timeout` が `Duration::ZERO` である
- **WHEN** flush driver が flush session を開始する
- **THEN** driver は無限待機しない
- **AND** flush は即時 timeout として扱われ、shutdown または DeathWatch notification の後続処理へ進む

#### Scenario: core does not read wall clock

- **WHEN** `modules/remote-core/src/association/` と `modules/remote-core/src/watcher/` の flush timeout 処理を検査する
- **THEN** `Instant::now()`、`SystemTime::now()`、`std::time::` を直接参照しない
- **AND** monotonic millis は std adaptor から入力される
