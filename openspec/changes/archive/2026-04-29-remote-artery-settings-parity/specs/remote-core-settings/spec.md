## ADDED Requirements

### Requirement: Artery advanced settings surface

`fraktor_remote_core_rs::core::config::RemoteConfig` は Pekko Artery advanced settings の responsibility parity に必要な large-message、inbound restart、compression の設定を型付き builder と accessor で保持する SHALL。これらの設定は fraktor-rs 独自 wire format の設定 surface であり、Pekko Artery wire protocol との byte compatibility を意味しない。

#### Scenario: large-message 設定を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、large-message 用 builder を呼ぶ
- **THEN** outbound large-message queue size と large-message destinations が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: invalid large-message queue size を拒否する

- **WHEN** outbound large-message queue size に `0` を指定する
- **THEN** `RemoteConfig` は invalid queue size として拒否し、zero-sized queue を保持しない

#### Scenario: inbound restart budget を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、inbound restart 用 builder を呼ぶ
- **THEN** inbound restart timeout と inbound max restarts が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: compression 設定 surface を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、compression 用 builder を呼ぶ
- **THEN** compression settings が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: wire-level compression は有効化しない

- **WHEN** compression settings を `RemoteConfig` に設定する
- **THEN** `core/wire` の PDU encoding、TCP framing、compression table の wire 表現は変更されず、設定保持だけが行われる

#### Scenario: no_std 境界を維持する

- **WHEN** `modules/remote-core/src/core/config/` 配下の import を検査する
- **THEN** `use std::` を含む行は存在せず、advanced settings は `core` と `alloc` の範囲で表現されている
