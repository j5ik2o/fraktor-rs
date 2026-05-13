## MODIFIED Requirements

### Requirement: remote-adaptor-std は remote-core の代替 lifecycle 入口を定義してはならない

`fraktor-remote-core-rs` は remote lifecycle の標準意味論として `Remote` / `RemoteShared` / `Remoting` port を提供しなければならない（MUST）。`fraktor-remote-adaptor-std-rs` は `RemoteTransport` の具象実装、actor system 配線、std runtime task orchestration を提供する adapter crate であり、`remote-core::Remote` と競合する lifecycle semantics を public API として定義してはならない（MUST NOT）。std adapter は ActorSystem lifecycle に接続された内部実装として core lifecycle operation を呼び出してよい（MAY）が、通常利用者に別 lifecycle sequence を書かせてはならない（MUST NOT）。

#### Scenario: std adaptor は StdRemoting 相当の public wrapper を持たない

- **WHEN** `modules/remote-adaptor-std/src/std` 配下の public 型と re-export を検査する
- **THEN** `StdRemoting` または同等の remote lifecycle wrapper は存在しない
- **AND** remote lifecycle の状態遷移と意味論は `remote-core::Remote` / `RemoteShared` に残る
- **AND** user-facing application code は `remote-core::Remote` を startup sequence として直接操作しない

#### Scenario: std adaptor は Port 実装と配線だけを提供する

- **WHEN** 利用者が std 環境で remote lifecycle を開始する
- **THEN** `TcpRemoteTransport` が `RemoteTransport` port 実装として core `RemoteShared` に差し込まれる
- **AND** `RemotingExtensionInstaller` は ActorSystem lifecycle に接続された adapter として core lifecycle operation を内部で呼ぶ
- **AND** `RemotingExtensionInstaller` は `remote.start()` / `spawn_run_task()` / `shutdown_and_join()` を通常利用者が順に呼ぶ別 lifecycle API として提供してはならない

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
