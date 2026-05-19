## MODIFIED Requirements

### Requirement: detach は close request と finalizer handoff を orchestrate する

`MessageDispatcherShared::detach` は、mailbox を terminal 状態へ遷移させる orchestration を提供しなければならない（MUST）。ただし detach caller が常に direct cleanup owner であってはならない（MUST NOT）。

detach は次を行う:

- mailbox に close request を立てる
- finalizer election を試みる
- finalizer を獲得した場合のみ immediate finalize を実行する
- finalizer を獲得できない場合は in-flight runner へ cleanup ownership を委譲する

#### Scenario: idle mailbox detach は caller finalizer で完結する
- **GIVEN** mailbox が running ではなく idle である
- **WHEN** `MessageDispatcherShared::detach(&self, actor)` を呼ぶ
- **THEN** detach caller は finalizer を獲得できる
- **AND** queue cleanup は caller 側で即時完了する

#### Scenario: running mailbox detach は runner に cleanup ownership を委譲する
- **GIVEN** mailbox が既に `run()` 中である
- **WHEN** `MessageDispatcherShared::detach(&self, actor)` を呼ぶ
- **THEN** detach caller は in-flight runner の終了待ちで block しない
- **AND** cleanup ownership は runner に委譲される
- **AND** detach path 自身は queue drain を direct には行わない

#### Scenario: detach は delayed shutdown 判定を維持する
- **WHEN** detach orchestration が finalizer handoff に変わる
- **THEN** dispatcher の inhabitants 減算と delayed shutdown 判定の契約は維持される
- **AND** mailbox cleanup ownership の変更によって scheduler 登録責務は `MessageDispatcherShared` から漏れない

