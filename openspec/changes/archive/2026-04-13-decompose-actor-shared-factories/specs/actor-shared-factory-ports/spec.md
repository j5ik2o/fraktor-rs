## ADDED Requirements

### Requirement: actor runtime の shared wrapper 構築 Port は 1責務1trait で分離されなければならない

actor runtime の shared wrapper / shared state を構築する Port 契約は、変更理由ごとに独立した factory trait として定義されなければならない（MUST）。dispatcher、executor、shared queue、actor-ref sender、event stream、actor-cell runtime state を単一の `ActorSharedFactory` に集約してはならない（MUST NOT）。

#### Scenario: Port は生成対象ごとに独立している
- **WHEN** actor runtime の shared factory 契約を確認する
- **THEN** `ExecutorSharedFactory`、`MessageDispatcherSharedFactory`、`SharedMessageQueueFactory`、`ActorRefSenderSharedFactory`、`EventStreamSharedFactory`、`EventStreamSubscriberSharedFactory`、`ActorSharedLockFactory`、`ActorCellStateSharedFactory`、`ReceiveTimeoutStateSharedFactory`、`MessageInvokerSharedFactory`、`MailboxSharedSetFactory` が独立 trait として存在する
- **AND** それぞれの trait は自分の生成責務だけを持つ

#### Scenario: 各 factory trait の public API は `create` に統一される
- **WHEN** 各 shared factory trait のメソッドを確認する
- **THEN** public API は `create(...)` という単一メソッド名に統一される
- **AND** trait 名が生成対象を表現し、メソッド名に生成対象名を重複させない

#### Scenario: 利用側は必要な Port だけに依存する
- **WHEN** actor runtime の wiring を確認する
- **THEN** dispatcher / executor / actor-ref / event stream / actor-cell は、自分が必要とする個別 factory trait だけに依存する
- **AND** 単一 `ActorSharedFactory` やそれに等価な総称 trait に依存しない
- **AND** Port 分割のためだけの新しい God Factory struct は導入しない
