# actor-core-extension-installers Specification

## Purpose
TBD - created by archiving change complete-remote-delivery-through-adaptor. Update Purpose after archive.
## Requirements
### Requirement: caller-retained shared extension installer

`ExtensionInstallers` は、caller が同じ installer handle を保持したまま `ActorSystemConfig::with_extension_installers` に登録できる shared installer 登録経路を提供しなければならない（MUST）。stateful installer を使う application code は、`ActorSystem` 作成後に `ExtensionInstaller::install(&system)` を直接呼ばなくても、bootstrap-time install と post-install control を両立できなければならない（MUST）。

#### Scenario: shared installer handle を config に登録できる

- **GIVEN** caller が `RemotingExtensionInstaller` の shared handle を作成している
- **WHEN** caller がその handle を `ExtensionInstallers` に登録し、`ActorSystemConfig::with_extension_installers` 経由で `ActorSystem::create_with_config` に渡す
- **THEN** actor system bootstrap は登録された同じ installer state に対して `install(system)` を実行する
- **AND** `ActorSystem::create_with_config` が成功した後、caller は保持している handle から install 済み state を観測できる
- **AND** application `main` は `installer.install(&system)` を直接呼ぶ必要がない

#### Scenario: stateful installer を別 state に分離しない

- **WHEN** shared installer handle を registry に登録する
- **THEN** registry が install する対象と caller が保持する対象は同じ underlying installer state である
- **AND** caller が保持する handle の post-install query は `NotStarted` 相当の未 install 状態を返してはならない

#### Scenario: install order は ActorSystem bootstrap に残る

- **WHEN** `ActorSystem::create_with_config` が `ActorSystemConfig` から extension installers を取り出す
- **THEN** extension installers は actor system bootstrap 中に実行される
- **AND** user-facing code は bootstrap 後に extension install order を手動で再現しない
