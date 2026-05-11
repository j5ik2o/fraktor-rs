## MODIFIED Requirements

### Requirement: Artery advanced settings surface

`fraktor_remote_core_rs::config::RemoteConfig` は Pekko Artery advanced settings の responsibility parity に必要な large-message、lane、compression の設定を型付き builder と accessor で保持する SHALL。これらの設定は fraktor-rs 独自 wire format の設定 surface であり、Pekko Artery wire protocol との byte compatibility を意味しない。

large-message と lane 設定は、設定保持だけでなく `Association` および std TCP transport の送受信処理へ反映されなければならない (MUST)。compression 設定はこの capability では保持のみとし、wire-level compression は有効化してはならない (MUST NOT)。

#### Scenario: large-message 設定を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、large-message 用 builder を呼ぶ
- **THEN** outbound large-message queue size と large-message destinations が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: invalid large-message queue size を拒否する

- **WHEN** outbound large-message queue size に `0` を指定する
- **THEN** `RemoteConfig` は invalid queue size として拒否し、zero-sized queue を保持しない

#### Scenario: large-message settings は Association enqueue policy に渡される

- **WHEN** `Association::from_config(local, remote, &config)` を呼ぶ
- **THEN** `config.large_message_destinations()` と `config.outbound_large_message_queue_size()` は association の enqueue policy に反映される

#### Scenario: lane settings を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、`with_inbound_lanes` / `with_outbound_lanes` を呼ぶ
- **THEN** inbound lane count と outbound lane count が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: invalid lane count を拒否する

- **WHEN** inbound lane count または outbound lane count に `0` を指定する
- **THEN** `RemoteConfig` は invalid lane count として拒否し、zero lane を保持しない

#### Scenario: lane settings は TCP transport construction に渡される

- **WHEN** `TcpRemoteTransport::from_config(system_name, config)` を呼ぶ
- **THEN** `config.inbound_lanes()` と `config.outbound_lanes()` は transport の inbound dispatch / outbound writer lane 構成に反映される

#### Scenario: compression 設定 surface を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、compression 用 builder を呼ぶ
- **THEN** compression settings が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: wire-level compression は有効化しない

- **WHEN** compression settings を `RemoteConfig` に設定する
- **THEN** `core/wire` の PDU encoding、TCP framing、compression table の wire 表現は変更されない
- **AND** transport は payload bytes を compression table によって置換しない

#### Scenario: no_std 境界を維持する

- **WHEN** `modules/remote-core/src/config/` 配下の import を検査する
- **THEN** `use std::` を含む行は存在せず、advanced settings は `core` と `alloc` の範囲で表現されている
