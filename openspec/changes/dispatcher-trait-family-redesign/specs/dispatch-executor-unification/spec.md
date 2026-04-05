## MODIFIED Requirements

### Requirement: executor 系は internal backend primitive としてのみ扱われる

`DispatchExecutor`、`DispatchExecutorRunner`、`TokioExecutor`、`ThreadedExecutor` は dispatcher public abstraction の中心ではなく、internal backend primitive としてのみ扱われなければならない。

#### Scenario: public dispatcher surface は executor 系を主語にしない
- **WHEN** dispatcher 関連の public API を確認する
- **THEN** public に選択される概念は `Dispatcher` / `DispatcherProvider` である
- **AND** `DispatchExecutor` と `DispatchExecutorRunner` は public concept の主語として現れない

#### Scenario: backend primitive は internal realization として存在してよい
- **WHEN** std adapter の dispatcher 実装を確認する
- **THEN** `TokioExecutor` や `ThreadedExecutor` は internal backend primitive として存在してよい
- **AND** public policy family 名として `TokioDispatcher` や `ThreadDispatcher` は公開されない

#### Scenario: executor 系を経由しないと dispatcher を選択できない API は存在しない
- **WHEN** actor / system の dispatcher selection API を確認する
- **THEN** caller は executor 型や runner 型を直接指定せずに dispatcher を選択できる
- **AND** executor 系の公開 re-export は必須条件にならない
