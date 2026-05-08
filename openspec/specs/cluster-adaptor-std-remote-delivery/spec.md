# cluster-adaptor-std-remote-delivery Specification

## Purpose
TBD - created by archiving change complete-remote-delivery-through-adaptor. Update Purpose after archive.
## Requirements
### Requirement: cluster std integration は remoting event subscription を保持する

`cluster-adaptor-std` の remoting lifecycle subscription は、topology auto-detection が必要な期間だけ有効でなければならない（MUST）。`EventStreamSubscription` は drop 時に unsubscribe するため、helper 内の `_subscription` ローカル変数に保持して return してはならない（MUST NOT）。

#### Scenario: subscribe helper は即 unsubscribe しない

- **WHEN** `subscribe_remoting_events(provider)` または後継 API を呼ぶ
- **THEN** 返された subscription guard が caller または provider state に保持される
- **AND** helper return 後に publish された `RemotingLifecycleEvent::Connected` / `Quarantined` が provider に届く

#### Scenario: connected event が topology を更新する

- **GIVEN** provider が started である
- **AND** remoting event subscription が active である
- **WHEN** event stream に `RemotingLifecycleEvent::Connected { authority, .. }` が publish される
- **THEN** provider は `handle_connected(authority)` 相当を実行する
- **AND** topology update event が観測できる

#### Scenario: subscription lifetime は明示される

- **WHEN** provider または caller が remoting lifecycle events を不要にする
- **THEN** subscription guard は意図的に drop される
- **AND** 以後の remoting events は topology を更新しない

### Requirement: cluster 向け remote delivery は actor-core provider resolution 経由で証明する

cluster std integration は remote actor ref を直接 adapter 内部型から構築してはならない（MUST NOT）。`ClusterApi::get` / `GrainRef` または既存の cluster-facing remote entry point は actor-core provider resolution を通じて remote actor ref を取得し、std remote adapter delivery path に到達しなければならない（MUST）。

#### Scenario: cluster remote reference が std remote adapter に到達する

- **GIVEN** std remote を install して started にした 2 つの actor system がある
- **AND** サポート対象 payload serialization が設定されている
- **AND** 必要な TCP peer connection が確立済みである
- **WHEN** cluster-facing API が remote target を resolve し、サポート対象 payload を送信する
- **THEN** call path は actor-core provider resolution を通る
- **AND** `RemoteActorRefSender` pushes `RemoteEvent::OutboundEnqueued`
- **AND** peer system は inbound local delivery bridge 経由で payload を受信する

#### Scenario: 未サポート payload は報告される

- **WHEN** cluster-facing API が std remote payload codec でサポートされない payload を送信しようとする
- **THEN** failure は caller または test harness から観測できる
- **AND** adapter は empty bytes や debug text を黙って送信しない
