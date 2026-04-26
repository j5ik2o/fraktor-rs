## ADDED Requirements

### Requirement: `pipe_to_self` / `pipe_to` は canonical future-to-message adapter として維持される

actor runtime は、`Future` の完了結果を actor message に変換する canonical adapter として、既存 `ActorContext::pipe_to_self`、`ActorContext::pipe_to`、`TypedActorContext::pipe_to_self`、`TypedActorContext::pipe_to` を維持しなければならない (MUST)。この change はそれらを handler が `Future` を返す別 contract で置き換えてはならない (MUST NOT)。

これらの adapter は Pekko typed の `ActorContext.pipeToSelf` と同じ設計意図を持つ。actor handler は同期的に future を起動し、future completion は mailbox message として actor に戻らなければならない (MUST)。

untyped `ActorContext::pipe_to_self` / `pipe_to` と `ContextPipeTask` が future-to-message の kernel contract でなければならない (MUST)。typed `TypedActorContext::pipe_to_self` / `pipe_to` は、その kernel contract に委譲する薄い wrapper でなければならない (MUST)。

#### Scenario: typed actor が future completion を self message として受け取る

- **GIVEN** typed actor が `TypedActorContext::pipe_to_self(future, map_ok, map_err)` を呼ぶ
- **WHEN** `future` が `Ok(value)` で完了する
- **THEN** `map_ok(value)` で生成された typed message が同じ actor の mailbox に enqueue される
- **AND** actor は通常の typed message handler でその message を受け取る

#### Scenario: typed actor が future failure を self message として受け取る

- **GIVEN** typed actor が `TypedActorContext::pipe_to_self(future, map_ok, map_err)` を呼ぶ
- **WHEN** `future` が `Err(error)` で完了する
- **THEN** `map_err(error)` で生成された typed message が同じ actor の mailbox に enqueue される
- **AND** actor は通常の typed message handler でその message を受け取る

### Requirement: typed pipe helper は `AnyMessage` を caller に露出しない

typed actor context の future-to-message adapter は、caller に `AnyMessage` 変換を直接要求してはならない (MUST NOT)。typed self delivery では、`TypedActorContext::pipe_to_self` が `AdaptMessage` 相当の actor-thread adapter を使い、`map_ok` / `map_err` の結果を actor protocol `M` へ変換しなければならない (MUST)。

typed adapter は future polling、waker registration、delivery retry、delivery failure 観測を独自に実装してはならない (MUST NOT)。それらは untyped kernel の `ContextPipeTask` / actor cell delivery 経路に委譲しなければならない (MUST)。

adapter 変換が失敗した場合、その失敗は actor の adapter failure 経路、log、または明示的 error として観測可能でなければならず (MUST)、無言で握りつぶしてはならない (MUST NOT)。

#### Scenario: caller は typed message だけを返す

- **GIVEN** typed actor が `TypedActorContext::pipe_to_self` を呼ぶ
- **WHEN** caller が `map_ok` / `map_err` を実装する
- **THEN** caller は actor protocol `M` または `AdapterError` を返す
- **AND** caller は `AnyMessage::new` を直接呼ぶ必要がない

#### Scenario: adapter failure は観測される

- **GIVEN** `map_ok` または `map_err` が `AdapterError` を返す
- **WHEN** pipe completion message が actor thread で adapter 実行される
- **THEN** failure は actor の adapter failure 経路または warn log で観測できる
- **AND** 成功したふりをして typed message を配送してはならない

### Requirement: actor API と typed behavior handler は同期 contract のまま維持される

`Actor::receive`、`TypedActor::receive`、`MessageInvoker::invoke`、`Behaviors::receive`、`Behaviors::receive_message` は同期 contract のまま維持されなければならない (MUST)。この change は actor / behavior handler が `Future` を返す新 contract を導入してはならない (MUST NOT)。

future が actor-owned state や `TypedActorContext` の mutable borrow を `.await` 跨ぎで保持できる API を提供してはならない (MUST NOT)。state 更新は completion message handler 内で同期的に行わなければならない (MUST)。

#### Scenario: behavior handler は future を返さない

- **WHEN** `Behaviors::receive_message` の handler signature を確認する
- **THEN** handler は `Result<Behavior<M>, ActorError>` を同期的に返す
- **AND** handler の戻り値は `Future` ではない

#### Scenario: async I/O は message 化される

- **GIVEN** actor が async I/O を開始する
- **WHEN** async I/O の結果で actor state を更新したい
- **THEN** actor は `pipe_to_self` で completion message を作る
- **AND** actor state は completion message handler 内で更新される

### Requirement: in-flight future completion は既存 delivery 観測経路に従う

`pipe_to_self` / `pipe_to` から起動された in-flight future の completion message は、通常の user message delivery と同じ観測経路に従わなければならない (MUST)。対象 actor が停止済み、mailbox closed、または delivery 不可能な場合、失敗は既存の send error / dead letter / log などの観測経路に記録されなければならない (MUST)。

restart 時の in-flight future は初期スコープでは runtime が暗黙 cancel してはならない (MUST NOT)。cancel や stale discard が必要な caller は generation token を message payload に含められなければならない (MUST)。

#### Scenario: actor 停止後の completion は観測可能に失敗する

- **GIVEN** future が actor 停止前に `pipe_to_self` で登録されている
- **WHEN** actor 停止後に future が完了する
- **THEN** completion message delivery は成功したふりをしない
- **AND** 失敗は send error、dead letter、または warn log として観測可能である

#### Scenario: restart 後の completion は通常 message として扱われる

- **GIVEN** future 起動後に actor が restart している
- **WHEN** future が完了する
- **THEN** completion message は現在の actor instance に通常 user message として配送される
- **AND** stale 判定が必要な actor は payload の generation token で discard できる
