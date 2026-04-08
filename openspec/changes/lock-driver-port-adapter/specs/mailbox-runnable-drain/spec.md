## MODIFIED Requirements

### Requirement: mailbox hot path は lock driver factory を通じて instrumentation driver を選択できる

mailbox hot path は lock 実装を `SpinSyncMutex` に固定していてはならない（MUST NOT）。`Mailbox` は、既存の run / enqueue / cleanup semantics を維持したまま、lock driver factory を通じて instrumentation driver を選択可能でなければならない（MUST）。

#### Scenario: Mailbox は lock driver 差し替え後も run contract を維持する
- **WHEN** `Mailbox` が instrumentation driver で構築されている
- **THEN** `run()` / enqueue / cleanup の既存 contract は変化しない
- **AND** close / suspend / reschedule の意味論も維持される

#### Scenario: actor-core hot path で debug driver を差し込める
- **WHEN** actor-core test configuration が debug driver factory を選択する
- **THEN** `ActorRefSenderShared` / `MessageDispatcherShared` / `ExecutorShared` / `Mailbox` が同じ driver family で構築できる
- **AND** 再入 deadlock を観測するための instrumentation seam が成立する
