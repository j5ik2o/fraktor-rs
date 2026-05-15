# remote-deployment-daemon Specification

## Purpose
TBD - created by archiving change remote-deployment-daemon. Update Purpose after archive.
## Requirements
### Requirement: std remote deployment daemon を起動する

`remote-adaptor-std` は remote create request を処理する deployment daemon を提供しなければならない（MUST）。daemon は actor system bootstrap 中に `RemotingExtensionInstaller` と同じ lifecycle で起動され、target node の local actor system state と deployable factory registry を使って actor を作成する。

#### Scenario: daemon は remoting install で利用可能になる

- **GIVEN** `RemotingExtensionInstaller` と `StdRemoteActorRefProviderInstaller` が actor system config に登録されている
- **WHEN** `ActorSystem::create_with_config` が成功する
- **THEN** std remote deployment daemon は inbound remote create request を処理できる状態で起動している

#### Scenario: daemon は system termination で停止する

- **GIVEN** std remote deployment daemon が起動済みである
- **WHEN** actor system termination が始まる
- **THEN** daemon task は新規 create request を受け付けない
- **AND** remoting shutdown と同じ lifecycle で停止する

### Requirement: create request は target node で actor を作成する

deployment daemon は inbound create request を受け取った場合、request の deployable factory id と payload を target node の registry で解決し、指定された target parent path と child name の下に local actor を作成しなければならない（MUST）。作成に成功した場合、daemon は created actor の canonical remote path を create success response として返さなければならない（MUST）。

#### Scenario: valid create request creates actor

- **GIVEN** target node に deployable factory id `echo` が登録済みである
- **AND** origin node が factory id `echo` と valid payload を持つ create request を送る
- **WHEN** deployment daemon が request を処理する
- **THEN** target node は local actor を作成する
- **AND** daemon は created actor の canonical remote path を success response で返す

#### Scenario: unknown factory id は failure response になる

- **GIVEN** target node に deployable factory id `missing` が登録されていない
- **WHEN** daemon が factory id `missing` の create request を処理する
- **THEN** daemon は actor を作成しない
- **AND** create failure response は `UnknownFactory` または同等の failure code を含む

#### Scenario: duplicate child name は failure response になる

- **GIVEN** target node の target parent 下に child name `worker` が既に存在する
- **WHEN** daemon が同じ child name の create request を処理する
- **THEN** daemon は duplicate local actor を作成しない
- **AND** create failure response は name conflict を表す failure code を含む

### Requirement: origin provider は create response を待って remote ref を返す

origin node の std remote provider は remote create request ごとに correlation id を発行し、matching create response または timeout まで pending state を保持しなければならない（MUST）。既存 spawn API が同期的であるため、provider hook は configured timeout で bounded な同期 wait を行う SHALL。success response を受け取った場合、provider は returned remote path を `StdRemoteActorRefProvider` の remote path resolution で materialize した `ActorRef` として返さなければならない（MUST）。

deployment create request frame は target daemon へ routing され、deployment create response frame は origin provider の pending response handler へ routing されなければならない（MUST）。response frame を target daemon の request handler や actor user message delivery に渡してはならない（MUST NOT）。

bounded wait は remote run task、TCP reader task、deployment response dispatcher task 自身を block してはならない（MUST NOT）。そのような context で待機が必要になった場合、provider hook は blocking-safe bridge を使うか、configuration failure として観測可能にしなければならない（MUST）。

#### Scenario: success response returns usable remote ref

- **GIVEN** origin node が remote deployment request を送信済みである
- **AND** target daemon が matching success response を返す
- **WHEN** origin provider が response を処理する
- **THEN** pending request は完了する
- **AND** caller は returned remote ref へ user message を送れる

#### Scenario: response routes to provider pending handler

- **GIVEN** origin provider が correlation id `42` の remote create request を pending として保持している
- **WHEN** TCP reader が correlation id `42` の create success frame を受信する
- **THEN** frame は deployment daemon request handler へ渡されない
- **AND** origin provider の pending response handler が該当 request を完了する

#### Scenario: timeout is observable

- **GIVEN** origin node が remote deployment request を送信済みである
- **AND** timeout まで matching response が到達しない
- **WHEN** provider timeout が発火する
- **THEN** pending request は failure として完了する
- **AND** caller は spawn failure を観測できる

#### Scenario: wait is bounded

- **GIVEN** origin node が remote deployment request を送信済みである
- **WHEN** matching response が configured timeout まで到達しない
- **THEN** provider hook は無期限に block しない
- **AND** pending request は timeout failure として完了する

#### Scenario: response dispatcher is not blocked by its own wait

- **WHEN** remote deployment hook が create response を待つ
- **THEN** wait は remote run task、TCP reader task、deployment response dispatcher task の進行を止めない
- **AND** response dispatcher が停止する context では configuration failure として観測可能になる

