## MODIFIED Requirements

### Requirement: actor-core hot path の shared wrapper は lock driver factory を選択可能にする

actor-core の再入 hot path に属する shared wrapper は、lock 実装を `SpinSyncMutex` に固定していてはならない（MUST NOT）。`ActorRefSenderShared`、`MessageDispatcherShared`、`ExecutorShared` は lock driver factory を通じて driver 差し替え可能でなければならない（MUST）。

#### Scenario: MessageDispatcherShared は hot path instrumentation driver で構築できる
- **WHEN** `MessageDispatcherShared` の内部 lock 構造を確認する
- **THEN** その内部 lock は `SpinSyncMutex` 固定ではない
- **AND** caller は factory 経由で instrumentation driver を選択できる
- **AND** public abstraction は引き続き `MessageDispatcher` trait と `MessageDispatcherShared` のままである
- **AND** public API に driver generic parameter は漏れない

#### Scenario: ExecutorShared は hot path instrumentation driver で構築できる
- **WHEN** `ExecutorShared` の内部 lock 構造を確認する
- **THEN** executor wrapper の内部 lock は driver 差し替え可能である
- **AND** trampoline の意味論は維持される

#### Scenario: ActorRefSenderShared は hot path instrumentation driver で構築できる
- **WHEN** `ActorRefSenderShared` の内部 lock 構造を確認する
- **THEN** per-actor sender lock は driver 差し替え可能である
- **AND** lock 解放後に schedule outcome を適用する再入安全性は維持される

#### Scenario: hot path genericization は `ActorCell` 内部で完結し public API へ漏れない
- **WHEN** actor-core hot path の型伝播を確認する
- **THEN** `DispatcherSender` / `ActorCell` は必要に応じて内部追従してよい
- **AND** `ActorSystem` / typed system / `ActorRef` の public API は nongeneric のままである
