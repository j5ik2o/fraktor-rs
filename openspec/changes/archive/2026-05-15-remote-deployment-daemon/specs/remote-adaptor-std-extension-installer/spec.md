## ADDED Requirements

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
