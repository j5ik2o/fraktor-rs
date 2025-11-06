## ADDED Requirements
### Requirement: Typed Actor System Wrapper
Typed 版アクターシステムは Untyped 実装を内包しつつ、ユーザーガーディアンから公開 API まで一貫してメッセージ型 `M` を保持しなければならない (MUST)。

#### Scenario: Typed system creates guardian via behavior adapter
- **GIVEN** `BehaviorGeneric<TB, M>` で記述されたユーザーガーディアンのルートビヘイビアが存在し
- **WHEN** `TypedActorSystemGeneric::new(behavior)` を呼び出すと
- **THEN** ビヘイビアは `PropsGeneric<TB>` に変換され Untyped `ActorSystemGeneric<TB>` を初期化し
- **AND** `TypedActorSystemGeneric` は `user_guardian_ref` などの API で `TypedActorRefGeneric<TB, M>` を返す

#### Scenario: Untyped system remains available
- **GIVEN** 既存の Untyped API を使用する利用者
- **WHEN** Untyped `ActorSystemGeneric` を直接生成する場合
- **THEN** Typed API の追加によってシグネチャや挙動が変わらず、従来コードは再コンパイルのみで動作する

### Requirement: Typed Message Flow Enforcement
Typed ActorRef/ChildRef/Context は `M` 型のメッセージのみを受け渡し、内部で `AnyMessageGeneric` へのエンコードを隠蔽しなければならない (MUST)。

#### Scenario: Typed actor ref only accepts M
- **GIVEN** `TypedActorRefGeneric<TB, M>` が存在し
- **WHEN** `tell` または `ask` に `M` 以外の型を渡そうとすると
- **THEN** コンパイルエラーとなり、`AnyMessageGeneric` の直接利用を避けられる

#### Scenario: Typed to untyped escape hatch
- **GIVEN** Typed API から Untyped 拡張を利用する必要がある場合
- **WHEN** `TypedActorRefGeneric::into_untyped()` のようなヘルパーを呼び出すと
- **THEN** 包んでいる `ActorRefGeneric<TB>` を取得でき、既存 Untyped API を引き続き呼び出せる

### Requirement: Typed Actor Behavior Lifecycle
Typed Actor は `TypedActorContextGeneric<'a, TB, M>` を介して spawn/reply/watch 等を行い、ランタイムの lifecycle と互換でなければならない (MUST)。

#### Scenario: Typed behavior handles message and replies
- **GIVEN** `TypedActor` が `M` を受け取る `receive` 実装を提供し
- **WHEN** `TypedActorContextGeneric::reply` を用いて返信する場合
- **THEN** `AnyMessageGeneric` への変換は内部で行われ、呼び出し側は `M` を渡すだけでよい

#### Scenario: Typed context spawns typed children
- **GIVEN** 親アクターが `TypedActorContextGeneric<'_, TB, M>` を受け取り
- **WHEN** `spawn_child` に子アクターの `BehaviorGeneric<TB, C>` を渡すと
- **THEN** 子の `TypedChildRefGeneric<TB, C>` が得られ、`stop/resume/watch` 等の操作も Typed ラッパー経由で利用できる

#### Scenario: Runtime invariants preserved
- **GIVEN** Typed API で spawn/stop/watch を行った場合
- **WHEN** ランタイム内部（Untyped）に渡るとき
- **THEN** 既存の `ActorSystemGeneric` / `ActorCellGeneric` の挙動を変えずにメッセージ処理が完了する
