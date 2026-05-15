# remote-adaptor-std-extension-installer Specification

## Purpose
TBD - created by archiving change complete-remote-delivery-through-adaptor. Update Purpose after archive.
## Requirements
### Requirement: RemotingExtensionInstaller は config install 経路で使える

`RemotingExtensionInstaller` は `ActorSystemConfig::with_extension_installers` 経由で install できなければならない（MUST）。caller が `TcpRemoteTransport` と `RemoteConfig` から作成した installer を actor system config に渡した時点で、std remote lifecycle は ActorSystem / installer 側で開始・実行・停止されなければならない（MUST）。通常利用者の application code は install 後に `installer.remote()` で `RemoteShared` を取り出して `remote.start()` を呼んではならず（MUST NOT）、remote run task の起動や shutdown join のために `spawn_run_task()` / `shutdown_and_join()` を直接呼んではならない（MUST NOT）。

#### Scenario: config install 後に remote lifecycle が内部で開始される

- **GIVEN** caller が `TcpRemoteTransport` と `RemoteConfig` から `RemotingExtensionInstaller` の shared handle を作成している
- **AND** caller がその handle を `ExtensionInstallers` に登録している
- **WHEN** caller が `ActorSystemConfig::with_extension_installers(installers)` を使って `ActorSystem::create_with_config` を呼ぶ
- **THEN** remote extension は actor system bootstrap 中に install される
- **AND** installer または ActorSystem lifecycle hook は core の `RemoteShared::start()` または同等の lifecycle operation を内部で呼ぶ
- **AND** std adapter は install 時に作成した `RemoteEventReceiver` を使って run task を起動する
- **AND** caller は startup sequence として `installer.remote()?.start()` または `installer.spawn_run_task()` を呼ばない

#### Scenario: remote shutdown は ActorSystem termination に接続される

- **GIVEN** `RemotingExtensionInstaller` が config install 経路で install 済みである
- **AND** std remote run task が adapter 内部で起動済みである
- **WHEN** caller が ActorSystem termination を要求する
- **THEN** installer または ActorSystem lifecycle hook は core の shutdown semantics を内部で呼ぶ
- **AND** adapter は remote event loop を wake し、tokio `JoinHandle` の完了を観測する
- **AND** caller は通常利用 path で `installer.shutdown_and_join().await` を呼ばない

#### Scenario: retained installer handle は startup API ではない

- **WHEN** caller が config 登録のために installer handle を保持している
- **THEN** その handle は provider installer 連携、診断、内部テストに使えてよい
- **AND** public showcase / docs は `installer.remote()` を remote startup API として示さない
- **AND** `remote.addresses()` の確認は application `main` ではなく core / adapter tests で行う

#### Scenario: remote lifecycle showcase は direct lifecycle calls を含まない

- **WHEN** `showcases/std/legacy/remote_lifecycle/main.rs` または後継 remote lifecycle showcase を検査する
- **THEN** showcase は `ExtensionInstallers` を作り、`ActorSystemConfig::with_extension_installers` に渡している
- **AND** showcase は `installer.install(&system)` または同等の direct install call を含まない
- **AND** showcase は `installer.remote()`、`remote.start()`、`spawn_run_task()`、`shutdown_and_join()` を remote lifecycle 手順として含まない

### Requirement: RemotingExtensionInstaller は deployment daemon lifecycle を所有する

`RemotingExtensionInstaller` は config install 経路で remote extension、watcher task、flush gate と同じ lifecycle に deployment daemon を接続しなければならない（MUST）。caller は通常利用 path で deployment daemon を手動 start してはならない（MUST NOT）。

#### Scenario: config install starts deployment daemon

- **GIVEN** caller が remoting installer と remote actor-ref provider installer を `ActorSystemConfig` に登録している
- **WHEN** actor system bootstrap が完了する
- **THEN** deployment daemon task は adapter 内部で起動済みである
- **AND** caller は deployment daemon の public start method を呼ばない

#### Scenario: shutdown aborts deployment daemon

- **GIVEN** deployment daemon task が起動済みである
- **WHEN** remote shutdown または actor system termination が実行される
- **THEN** deployment daemon task は停止される
- **AND** pending create request は failure または cancellation として観測可能になる

### Requirement: installer は deployment dependencies を共有する

deployment daemon は actor system handle、serialization extension、remote event sender、monotonic epoch、local address、deployable factory registry を install 時に受け取らなければならない（MUST）。daemon は独自の serialization registry または別 remoting instance を作ってはならない（MUST NOT）。

#### Scenario: daemon uses actor system serialization extension

- **WHEN** deployment daemon が create request payload を deserialize する
- **THEN** daemon は actor system に登録済みの serialization extension を使う
- **AND** daemon 専用の `SerializationRegistry::from_setup` 相当を新規構築しない
