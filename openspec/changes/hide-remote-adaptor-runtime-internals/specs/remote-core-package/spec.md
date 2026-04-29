## MODIFIED Requirements

### Requirement: remote-adaptor-std は remote-core の代替 lifecycle 入口を定義してはならない

`fraktor-remote-core-rs` は remote lifecycle の標準 API として `Remote` と `Remoting` port を提供しなければならない（MUST）。`fraktor-remote-adaptor-std-rs` は `RemoteTransport` の具象実装と actor system 配線を提供する adapter crate であり、`remote-core::Remote` と競合する lifecycle wrapper を public API として定義してはならない（MUST NOT）。

#### Scenario: std adaptor は StdRemoting 相当の public wrapper を持たない

- **WHEN** `modules/remote-adaptor-std/src/std` 配下の public 型と re-export を検査する
- **THEN** `StdRemoting` または同等の remote lifecycle wrapper は存在しない
- **AND** `remote-core::Remote` が lifecycle の利用口である

#### Scenario: std adaptor は Port 実装と配線だけを提供する

- **WHEN** 利用者が std 環境で remote lifecycle を開始する
- **THEN** `TcpRemoteTransport` が `RemoteTransport` port 実装として `Remote` に差し込まれる
- **AND** `RemotingExtensionInstaller` は `Remote` を actor system extension として登録するだけで、別 lifecycle API を提供しない

### Requirement: remote-core の公開 API は adapter runtime internal に依存してはならない

`fraktor-remote-core-rs` は `fraktor-remote-adaptor-std-rs` の runtime internal 型に依存してはならない（MUST NOT）。`Remote` は `RemoteTransport` port だけに依存し、std transport や provider bridge の具象型を型シグネチャに露出してはならない（MUST NOT）。

#### Scenario: Remote は TcpRemoteTransport を型パラメータとして露出しない

- **WHEN** `modules/remote-core/src/core/extension/remote.rs` を検査する
- **THEN** `pub struct Remote<T>` ではなく非ジェネリックな `pub struct Remote` である
- **AND** `Remote` の public method signature に `TcpRemoteTransport` は現れない

#### Scenario: remote-core は remote-adaptor-std に依存しない

- **WHEN** `modules/remote-core/Cargo.toml` の dependencies を検査する
- **THEN** `fraktor-remote-adaptor-std-rs` への依存は存在しない
- **AND** `modules/remote-core/src` 配下に `fraktor_remote_adaptor_std_rs` への import は存在しない
