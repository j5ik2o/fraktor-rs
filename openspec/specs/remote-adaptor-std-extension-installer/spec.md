# remote-adaptor-std-extension-installer Specification

## Purpose
TBD - created by archiving change complete-remote-delivery-through-adaptor. Update Purpose after archive.
## Requirements
### Requirement: RemotingExtensionInstaller は config install 経路で使える

`RemotingExtensionInstaller` は `ActorSystemConfig::with_extension_installers` 経由で install できなければならない（MUST）。caller は install 後も同じ installer handle から `remote()` を取得し、`start()` 後に `spawn_run_task()` と `shutdown_and_join()` を呼べなければならない（MUST）。remote lifecycle の user-facing showcase は `installer.install(&system)` を直接呼んではならない（MUST NOT）。

#### Scenario: config install 後に remote handle を取得できる

- **GIVEN** caller が `TcpRemoteTransport` と `RemoteConfig` から `RemotingExtensionInstaller` の shared handle を作成している
- **AND** caller がその handle を `ExtensionInstallers` に登録している
- **WHEN** caller が `ActorSystemConfig::with_extension_installers(installers)` を使って `ActorSystem::create_with_config` を呼ぶ
- **THEN** remote extension は actor system bootstrap 中に install される
- **AND** caller が保持している installer handle の `remote()` は install 済み `RemoteShared` を返す
- **AND** caller はその `RemoteShared` に対して `start()` と `addresses()` を呼べる

#### Scenario: remote run task lifecycle は retained handle から制御できる

- **GIVEN** `RemotingExtensionInstaller` が config install 経路で install 済みである
- **AND** caller が保持している installer handle から取得した `RemoteShared` は `start()` 済みである
- **WHEN** caller が同じ installer handle から `spawn_run_task()` を呼ぶ
- **THEN** installer は install 時に作成した `RemoteEventReceiver` を使って run task を起動する
- **AND** caller は同じ handle から `shutdown_and_join().await` を呼んで run task を停止できる

#### Scenario: remote lifecycle showcase は direct install を含まない

- **WHEN** `showcases/std/legacy/remote_lifecycle/main.rs` を検査する
- **THEN** showcase は `ExtensionInstallers` を作り、`ActorSystemConfig::with_extension_installers` に渡している
- **AND** showcase は `installer.install(&system)` または同等の direct install call を含まない
- **AND** surface test はこの usage を検証する
