## MODIFIED Requirements

### Requirement: actor runtime の shared wrapper 構築は個別 factory Port 境界に集約されなければならない

actor runtime が使う dispatcher、executor、actor-ref sender、mailbox lock bundle、shared queue の shared wrapper 構築は、単一の God Factory ではなく変更理由ごとに分離された個別 factory Port 境界に集約されなければならない（MUST）。actor-system 管理下の production wiring が `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` / `SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(...)` のような固定 concrete driver 指定や、`*::new_with_builtin_lock(...)` のような fixed-family helper alias を直接行ってはならない（MUST NOT）。

#### Scenario: dispatcher / executor / shared queue は個別 Port から materialize される
- **WHEN** actor system が dispatcher、executor、balancing dispatcher 用 shared queue を構築する
- **THEN** `MessageDispatcherSharedFactory`、`ExecutorSharedFactory`、`SharedMessageQueueFactory` という個別 Port から materialize される
- **AND** dispatcher wiring は単一 `ActorSharedFactory` のような多責務 trait に依存しない
- **AND** caller は concrete lock family 名を直接指定しない

#### Scenario: actor-ref sender と mailbox bundle は dispatcher factory に混在しない
- **WHEN** actor runtime が actor-ref sender shared wrapper または mailbox shared bundle を構築する
- **THEN** それぞれ `ActorRefSenderSharedFactory` と `MailboxSharedSetFactory` を通して構築される
- **AND** dispatcher / executor 向け Port に actor-ref / mailbox の責務を混在させない

#### Scenario: debug family 選択時に subsystem ごとの Port へ同じ family を適用できる
- **WHEN** actor system が debug 用の shared factory 実装を設定して起動する
- **THEN** dispatcher、executor、shared queue、actor-ref sender、mailbox shared set は対応する個別 Port を通じて同じ family で構築される
- **AND** ある subsystem だけが builtin spin backend に固定される silent bypass は存在しない
