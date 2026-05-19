## ADDED Requirements

### Requirement: write-once フィールドは spin::Once で初期化後の read を lock-free にしなければならない

write-once パターン（初期化時に 1 回セット、以後は読み取りのみ）と判定されたフィールドは、`SharedLock<T>` / `SharedRwLock<T>` ではなく `spin::Once<T>` を使用しなければならない（MUST）。

#### Scenario: CoordinatedShutdown.reason の read path が atomic load のみ

- **GIVEN** `CoordinatedShutdown` の `reason` フィールドが `spin::Once<CoordinatedShutdownReason>` として定義されている
- **WHEN** `run()` で `call_once` により理由がセットされた後、`shutdown_reason()` が呼ばれる
- **THEN** Mutex acquire は発生しない
- **AND** `spin::Once::get()` (atomic load) のみで値を取得できる

#### Scenario: ContextPipeWakerHandleShared.inner の read path が atomic load のみ

- **GIVEN** `ContextPipeWakerHandleShared` の `inner` フィールドが `spin::Once<ContextPipeWakerHandle>` として定義されている
- **WHEN** コンストラクタで `spin::Once::initialized(handle)` により初期化された後、`wake()` が呼ばれる
- **THEN** Mutex acquire は発生しない
- **AND** `spin::Once::get()` (atomic load) のみで値を取得できる

#### Scenario: write-once 検証に不合格の型は除外する

- **GIVEN** `*Shared` 型のコードを読解する
- **WHEN** 初期化後に `with_write` / `with_lock` で内部オブジェクトの `&mut self` メソッドを呼ぶ箇所が存在する
- **THEN** その型は write-once ではないと判定する
- **AND** `spin::Once` 置換の対象から除外する

### Requirement: single-thread-access パターンは本 change のスコープ外

dispatcher thread からのみアクセスされる mutable state（`ActorCellStateShared`, `ReceiveTimeoutStateShared`, `ActorShared`）は本 change では変更しない（MUST NOT）。これらは `Send + Sync` 制約のため `RefCell` 化に設計変更を伴い、別 change で検討する。

### Requirement: 除外済み 5 型は変更しない

以下の 5 型はコード読解により write-once ではないと判定済みであり、本 change では変更しない（MUST NOT）:

- `MiddlewareShared` — `before_user`/`after_user` が `&mut self` で毎メッセージ呼ばれる
- `ActorRefProviderHandleShared` — temp actor 管理の変更操作が ongoing
- `ExecutorShared` — `Executor::execute(&mut self)` が毎タスクで呼ばれる
- `MessageDispatcherShared` — `attach`/`detach`/`dispatch` が hot path で実行
- `DeadLetterShared` — `record_send_error`/`record_entry` で追記
