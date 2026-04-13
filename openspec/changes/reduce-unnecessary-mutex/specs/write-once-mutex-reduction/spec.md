## ADDED Requirements

### Requirement: write-once Shared 型は spin::Once で初期化後の read を lock-free にしなければならない

write-once パターン（初期化時に 1 回セット、以後は読み取りのみ）と判定された `*Shared` 型は、`SharedLock<T>` / `SharedRwLock<T>` ではなく `spin::Once<T>` を使用しなければならない（MUST）。

#### Scenario: write-once Shared 型の read path が atomic load のみ

- **GIVEN** write-once と検証された `*Shared` 型
- **WHEN** 初期化完了後に read アクセスする
- **THEN** Mutex acquire / RwLock read-lock は発生しない
- **AND** `spin::Once::get()` (atomic load) のみで値を取得できる

#### Scenario: write-once 検証に不合格の型は除外する

- **GIVEN** `*Shared` 型のコードを読解する
- **WHEN** 初期化後に `with_write` / `with_lock` で値を変更する箇所が存在する
- **THEN** その型は write-once ではないと判定する
- **AND** `spin::Once` 置換の対象から除外する

### Requirement: single-thread-access パターンは本 change のスコープ外

dispatcher thread からのみアクセスされる mutable state（`ActorCellStateShared`, `ReceiveTimeoutStateShared`, `ActorShared`）は本 change では変更しない（MUST NOT）。これらは `Send + Sync` 制約のため `RefCell` 化に設計変更を伴い、別 change で検討する。
