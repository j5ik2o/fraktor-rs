## MODIFIED Requirements

### Requirement: Artery advanced settings surface

`fraktor_remote_core_rs::config::RemoteConfig` は Pekko Artery advanced settings の responsibility parity に必要な large-message、lane、inbound restart、compression の設定を型付き builder と accessor で保持する SHALL。これらの設定は fraktor-rs 独自 wire format の設定 surface であり、Pekko Artery wire protocol との byte compatibility を意味しない。

large-message と lane 設定は、設定保持だけでなく `Association` および std TCP transport の送受信処理へ反映されなければならない (MUST)。compression 設定は actor ref / serializer manifest compression table の sizing と advertisement scheduling に反映されなければならない (MUST)。payload bytes の圧縮はこの設定 surface の責務ではない (MUST NOT)。

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

#### Scenario: inbound restart budget を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、inbound restart 用 builder を呼ぶ
- **THEN** inbound restart timeout と inbound max restarts が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: compression 設定 surface を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、compression 用 builder を呼ぶ
- **THEN** compression settings が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: compression max は table sizing に反映される

- **GIVEN** `RemoteConfig` に `actor_ref_max = Some(64)` と `manifest_max = Some(32)` を持つ compression settings を設定する
- **WHEN** `TcpRemoteTransport::from_config(system_name, config)` を呼ぶ
- **THEN** transport の peer-local actor-ref compression table configuration は最大 64 entries として保持される
- **AND** peer-local manifest compression table configuration は最大 32 entries として保持される

#### Scenario: compression max None は local outbound を kind 単位で無効化する

- **GIVEN** `RemoteConfig` に `actor_ref_max = None` と `manifest_max = Some(32)` を持つ compression settings を設定する
- **WHEN** `TcpRemoteTransport::from_config(system_name, config)` を呼ぶ
- **THEN** actor-ref local outbound compression は disabled になる
- **AND** manifest local outbound compression は enabled のまま保持される

#### Scenario: advertisement interval は std transport timer に渡される

- **WHEN** `TcpRemoteTransport::from_config(system_name, config)` を呼ぶ
- **THEN** `config.compression_config().actor_ref_advertisement_interval()` と `manifest_advertisement_interval()` は transport の compression advertisement timer 構成に反映される

#### Scenario: payload bytes は compression table で置換しない

- **WHEN** compression settings を `RemoteConfig` に設定する
- **THEN** transport は serializer id / manifest / actor path metadata のみを compression table の対象にする
- **AND** serialized payload bytes を compression table entry または table reference に置換しない

#### Scenario: no_std 境界を維持する

- **WHEN** `modules/remote-core/src/config/` 配下の import を検査する
- **THEN** `use std::` を含む行は存在せず、advanced settings は `core` と `alloc` の範囲で表現されている
