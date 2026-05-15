## ADDED Requirements

### Requirement: std provider は RemoteScope deployment を remote create へ接続する

`StdRemoteActorRefProvider` またはその installer が actor-core remote deployment hook を登録し、remote-scoped spawn を remote create request へ変換しなければならない（MUST）。hook は existing actor-ref provider dispatch と同じ local address 判定を使い、loopback remote scope では actor-core に `UseLocalDeployment` 相当の outcome を返さなければならない（MUST）。

#### Scenario: remote scope creates request

- **GIVEN** child spawn が `Scope::Remote(remote_node)` を持つ deploy metadata を使う
- **AND** remote_node は local address と一致しない
- **WHEN** actor-core remote deployment hook が呼ばれる
- **THEN** std provider は target node 宛ての remote create request を enqueue する
- **AND** local provider には child actor creation を委譲しない

#### Scenario: loopback scope stays local

- **GIVEN** child spawn が `Scope::Remote(local_node)` を持つ deploy metadata を使う
- **AND** local_node は installed provider の local address と一致する
- **WHEN** actor-core remote deployment hook が呼ばれる
- **THEN** std provider は remote create request を送信しない
- **AND** hook は actor-core に local spawn path へ戻る outcome を返す
- **AND** local actor creation は actor-core の既存 local spawn path で行われる

### Requirement: provider pending request state は correlation id で管理する

std provider は outbound remote create request ごとに unique correlation id を割り当て、matching response を受信するまで pending request state を保持しなければならない（MUST）。response の correlation id が未知または期限切れの場合、provider はその response を successful spawn として扱ってはならない（MUST NOT）。

#### Scenario: matching response completes pending request

- **GIVEN** provider が correlation id `42` の remote create request を pending として保持している
- **WHEN** deployment daemon response が correlation id `42` で到着する
- **THEN** provider は該当 pending request だけを完了する

#### Scenario: stale response is rejected

- **GIVEN** provider に correlation id `7` の pending request が存在しない
- **WHEN** deployment daemon response が correlation id `7` で到着する
- **THEN** provider は response を spawn success として扱わない
- **AND** stale response は log または test-observable error path で観測可能になる
