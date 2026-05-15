# actor-core-remote-deployment Specification

## Purpose
TBD - created by archiving change remote-deployment-daemon. Update Purpose after archive.
## Requirements
### Requirement: RemoteScope spawn は remote deployment hook へ委譲する

actor-core は child spawn の deployment metadata が `Scope::Remote(RemoteScope)` を持つ場合、local actor cell を作成する前に installed remote deployment hook へ委譲しなければならない（MUST）。hook が remote create を成功させた場合、spawn result は target node 上で作成された actor を指す remote `ActorRef` / `ChildRef` 相当を返さなければならない（MUST）。

hook は actor-core の既存同期 spawn surface に合わせ、同期的に `RemoteCreated`、`UseLocalDeployment`、`Failed` 相当の outcome を返す SHALL。actor-core は `UseLocalDeployment` を受け取った場合のみ通常の local spawn path に進み、remote create request の送信や response 待機の詳細を知らない MUST。

remote deployment hook が未登録、target address が remote provider で解決できない、create response が失敗、または timeout した場合、actor-core は local actor へ fallback してはならず（MUST NOT）、`SpawnError` として観測可能にしなければならない（MUST）。

#### Scenario: RemoteScope child spawn は local cell を作らない

- **GIVEN** parent actor が `Deploy` registry で `Scope::Remote(remote_node)` を持つ child path を spawn する
- **AND** remote deployment hook が installed である
- **WHEN** child spawn が実行される
- **THEN** actor-core は local `ActorCell` をその child path に登録しない
- **AND** remote deployment hook に create request を委譲する
- **AND** spawn result は remote node 上の created actor path を指す ref になる

#### Scenario: hook 未登録は spawn failure になる

- **GIVEN** child deployment が `Scope::Remote(remote_node)` を持つ
- **AND** remote deployment hook が installed ではない
- **WHEN** child spawn が実行される
- **THEN** spawn は `SpawnError::InvalidProps` または同等の observable failure を返す
- **AND** actor-core は同じ child を local actor として生成しない

#### Scenario: loopback outcome は local spawn path に戻る

- **GIVEN** child deployment が local node を指す `Scope::Remote(local_node)` を持つ
- **AND** remote deployment hook が `UseLocalDeployment` outcome を返す
- **WHEN** child spawn が実行される
- **THEN** actor-core は remote create request を前提にせず、既存の local spawn path で child actor を生成する

### Requirement: deployable props は wire-safe metadata を持つ

remote deployment に使われる `Props` は target node が解決できる stable deployable factory id と、actor-core serialization で `SerializationCallScope::Remote` として serialize できる factory payload を持たなければならない（MUST）。remote deployment は arbitrary Rust closure、`Box<dyn ActorFactory>` の内部表現、または raw `Props` memory layout を wire payload として送ってはならない（MUST NOT）。

#### Scenario: deployable props は factory id と payload を expose する

- **WHEN** remote deployment 用の props metadata を検査する
- **THEN** metadata は deployable factory id を持つ
- **AND** metadata は actor-core serialization に渡せる payload を持つ
- **AND** closure pointer や raw factory trait object を含まない

#### Scenario: non-deployable props は remote spawn を拒否する

- **GIVEN** `Props::from_fn` で作られた local-only props がある
- **AND** deployable factory metadata が設定されていない
- **WHEN** その props を `Scope::Remote` で spawn する
- **THEN** spawn は observable failure を返す
- **AND** local fallback は実行されない

### Requirement: remote child lifecycle は DeathWatch に接続される

remote deployment により返された remote child ref は parent/child lifecycle の観測対象として扱われなければならない（MUST）。parent が remote child を watch する場合、既存 remote DeathWatch path を使い、target node の終了通知が parent に到達しなければならない（MUST）。

remote child に対する user messaging は underlying remote `ActorRef` で配送されなければならない（MUST）。一方で remote child stop / suspend / resume protocol はこの change の対象外である。remote child に対する `ChildRef::stop`、`suspend`、`resume` 相当の lifecycle command は local child と同一視してはならず（MUST NOT）、remote system-command protocol が追加されるまでは observable unsupported failure として返さなければならない（MUST）。

#### Scenario: remote child termination reaches parent

- **GIVEN** parent actor が remote child を spawn 済みである
- **WHEN** target node 上の child actor が停止する
- **THEN** parent は既存 remote DeathWatch 経路で termination を観測する
- **AND** actor-core は remote child を missing local cell として即時 terminated 扱いしない

#### Scenario: remote child stop is unsupported in this change

- **GIVEN** parent actor が remote child を spawn 済みである
- **WHEN** caller が remote child の `ChildRef::stop` 相当を呼ぶ
- **THEN** actor-core は local child stop として処理しない
- **AND** caller は unsupported remote lifecycle command を表す observable failure を受け取る

