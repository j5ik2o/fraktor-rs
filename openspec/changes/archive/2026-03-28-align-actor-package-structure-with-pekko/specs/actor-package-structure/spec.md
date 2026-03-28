## ADDED Requirements

### Requirement: actor core package は kernel と typed を最上位境界として分離される
`modules/actor/src/core` は、untyped runtime の責務を表す `kernel` と、typed API/typed runtime の責務を表す `typed` を最上位境界として持たなければならない。最上位 package は責務軸と型付け軸を混在させてはならず、この境界は MUST 明示される。

#### Scenario: core の最上位 package が二軸混在を解消する
- **WHEN** `modules/actor/src/core.rs` と `modules/actor/src/core/` の package 構造を確認する
- **THEN** core の最上位境界には `kernel` と `typed` が存在する
- **AND** typed 以外の untyped runtime package は `kernel` 配下に配置される

### Requirement: typed package は Pekko 対応の責務語彙で再編される
`modules/actor/src/core/typed` は、typed primitive と、Pekko Typed に対応する receptionist、pubsub、routing の責務 package に分割されなければならない。service discovery、topic pub/sub、router builder 群は root 直下に散在してはならず、対応する package に MUST 集約される。

#### Scenario: receptionist 関連型が receptionist package に集約される
- **WHEN** typed の service discovery 関連型を確認する
- **THEN** `Receptionist`、`ReceptionistCommand`、`ServiceKey`、`Listing` は `core/typed/receptionist/` 配下に配置される

#### Scenario: pubsub 関連型が pubsub package に集約される
- **WHEN** typed の topic 関連型を確認する
- **THEN** `Topic`、`TopicCommand`、`TopicStats` は `core/typed/pubsub/` 配下に配置される

#### Scenario: routing 関連型が routing package に集約される
- **WHEN** typed の router factory と router builder を確認する
- **THEN** `Routers`、`Resizer`、`GroupRouterBuilder`、`PoolRouterBuilder`、`BalancingPoolRouterBuilder`、`ScatterGatherFirstCompletedRouterBuilder`、`TailChoppingRouterBuilder` は `core/typed/routing/` 配下に配置される

### Requirement: typed root 公開面は typed primitive に限定される
`modules/actor/src/core/typed` の root 公開面は、typed actor primitive、behavior primitive、message adapter、props、scheduler、spawn protocol、supervision などの typed 基盤に限定されなければならない。receptionist、pubsub、routing の語彙は root 直下へ再 export されず、対応する package 経由で参照されなければならない。

#### Scenario: typed root から責務別語彙が除外される
- **WHEN** `modules/actor/src/core/typed.rs` の公開面を確認する
- **THEN** receptionist、pubsub、routing の型は root 直下の主要公開面として並ばない
- **AND** それらの型は `typed::receptionist`、`typed::pubsub`、`typed::routing` 経由で参照される
