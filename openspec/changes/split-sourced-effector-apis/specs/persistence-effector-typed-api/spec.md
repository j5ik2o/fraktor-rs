## MODIFIED Requirements

### Requirement: typed persistence は通常の `Behavior` ベース actor として実装できる

fraktor-rs の typed event-sourced persistence API は、ユーザー actor に `EventSourcedBehavior` 相当の専用 command handler / event handler DSL を強制してはならない (MUST NOT)。ユーザーは `Behaviors::setup`、`Behaviors::receive_message`、`Behaviors::receive_message_partial`、状態別 handler 関数を使って aggregate actor を実装できなければならない (MUST)。

typed event-sourced persistence API は `EventSourcedEffector::props(config, on_ready)` と低レベルの `EventSourcedEffector::from_config(config, on_ready)` を提供し、recovery 完了後に `on_ready(state, effector)` を呼び出して初期 `Behavior<M>` を生成しなければならない (MUST)。

typed persistence API は `modules/persistence-core-typed` / `fraktor-persistence-core-typed-rs` に配置しなければならない (MUST)。classic persistence 基盤は `modules/persistence-core-kernel` / `fraktor-persistence-core-kernel-rs` に配置し、actor runtime 依存は `fraktor-actor-core-kernel-rs` までに留めなければならない (MUST)。`fraktor-utils-core-rs` や no_std 対応の補助 crate への依存はこの actor runtime 境界の制約に含めない。

`fraktor-persistence-core-kernel-rs` は `fraktor-actor-core-typed-rs` に依存してはならない (MUST NOT)。`fraktor-persistence-core-typed-rs` だけが `fraktor-persistence-core-kernel-rs` と `fraktor-actor-core-typed-rs` を合成し、`Behavior`, `Behaviors`, `TypedActorContext`, `StashBuffer`, `TypedProps` と連携しなければならない (MUST)。

#### Scenario: recovery 後に state-specific behavior を開始できる

- **GIVEN** `EventSourcedEffectorConfig` に `initial_state` と `apply_event` が設定されている
- **WHEN** actor が起動し recovery が完了する
- **THEN** `on_ready(recovered_state, effector)` が呼び出される
- **AND** ユーザーは `recovered_state` の variant に応じて別々の `Behavior<M>` を返せる

#### Scenario: recovery 中の user command は stash される

- **GIVEN** actor が recovery 中である
- **WHEN** user command が mailbox に届く
- **THEN** effector wrapper はその command を stash する
- **AND** recovery 完了後に `on_ready` が返した behavior へ unstash する

#### Scenario: stash mailbox contract が明示される

- **GIVEN** effector wrapper が recovery / persist 中に command を stash する
- **WHEN** user が `EventSourcedEffector::props(config, on_ready)` で effector aggregate actor を spawn する
- **THEN** returned `TypedProps<M>` は `TypedProps::with_stash_mailbox()` 相当を適用済みである
- **AND** effector implementation は `Behaviors::with_stash` / `StashBuffer<M>` に基づいて stash / unstash する
- **AND** `from_config` を直接使う advanced caller は `TypedProps::from_behavior_factory(...).with_stash_mailbox()` 相当を明示する

#### Scenario: persistence kernel は typed crate に依存しない

- **WHEN** `modules/persistence-core-kernel/Cargo.toml` を確認する
- **THEN** dependency / dev-dependency に `fraktor-actor-core-typed-rs` は存在しない
- **AND** actor runtime dependency としては `fraktor-actor-core-kernel-rs` だけで classic persistence API を提供する

## REMOVED Requirements

### Requirement: `PersistenceEffector` は event persistence operation を提供する

**Reason**: `PersistenceEffector` という名前は persistence 全般を示すように見えるが、実装契約は event journal、snapshot、`apply_event` に基づく Event Sourcing 専用である。State Sourcing 用 Effector を兄弟 API として追加するため、旧名を public contract から削除する。

**Migration**: `PersistenceEffector`、`PersistenceEffectorConfig`、`PersistenceEffectorSignal`、`PersistenceEffectorMessageAdapter`、`PersistenceEffectorSignalAuth` をそれぞれ `EventSourcedEffector`、`EventSourcedEffectorConfig`、`EventSourcedEffectorSignal`、`EventSourcedEffectorMessageAdapter`、`EventSourcedEffectorSignalAuth` に置き換える。互換 alias は追加しない。

## ADDED Requirements

### Requirement: `EventSourcedEffector` は event persistence operation を提供する

typed event-sourced persistence API は `persist_event` と `persist_events` を提供しなければならない (MUST)。これらの operation は event を store actor または mode-specific store に保存し、保存成功後に callback を実行しなければならない (MUST)。

persist callback は operation ごとに一度だけ実行される one-shot callback でなければならない (MUST)。Rust API では `FnOnce` 相当を受け付け、command handler が作った new state を clone せず callback に move できなければならない (MUST)。

persist operation は保存完了前に user command を処理してはならない (MUST NOT)。保存待ち中の user command は `stash_capacity` に従って stash しなければならない (MUST)。保存成功後、callback が返した behavior へ stashed command を戻さなければならない (MUST)。

#### Scenario: 単一 event persist 成功後に callback が実行される

- **GIVEN** aggregate actor が command を処理して event `E1` を生成する
- **WHEN** actor が `effector.persist_event(ctx, E1, callback)` を呼ぶ
- **AND** store actor の reply が `EventSourcedEffectorSignal::PersistedEvents([E1])` に変換される
- **THEN** callback は `E1` を受け取って実行される
- **AND** callback が返した behavior が次の active behavior になる

#### Scenario: persist 中の command は保存成功まで処理されない

- **GIVEN** `persist_event` の effector signal を待っている
- **WHEN** 別の user command が届く
- **THEN** effector wrapper は command を stash する
- **AND** persist 成功 callback 完了後に unstash する

#### Scenario: 複数 event は batch として保存される

- **GIVEN** command が複数 event `[E1, E2, E3]` を生成する
- **WHEN** actor が `persist_events` を呼ぶ
- **THEN** store actor は event sequence を同一 persistence id に順序通り保存する
- **AND** callback は保存された event slice を順序通り受け取る

### Requirement: Event Sourcing Effector の public names は canonical name だけを公開する

typed event-sourced persistence API は `EventSourcedEffector`、`EventSourcedEffectorConfig`、`EventSourcedEffectorSignal`、`EventSourcedEffectorMessageAdapter` を canonical public API として公開しなければならない (MUST)。旧 `PersistenceEffector*` 名の type alias、deprecated item、compat module を公開してはならない (MUST NOT)。

internal store protocol は event-sourced semantics を表す名前へ揃えなければならない (MUST)。user message API は stable public signal と message adapter だけを使い、internal command / reply 型を要求してはならない (MUST NOT)。

#### Scenario: crate root は EventSourced names だけを re-export する

- **WHEN** crate user が `fraktor_persistence_core_typed_rs` を import する
- **THEN** `EventSourcedEffector`, `EventSourcedEffectorConfig`, `EventSourcedEffectorSignal`, `EventSourcedEffectorMessageAdapter` を利用できる
- **AND** `PersistenceEffector`, `PersistenceEffectorConfig`, `PersistenceEffectorSignal`, `PersistenceEffectorMessageAdapter` は利用できない

#### Scenario: aggregate message は event-sourced effector signal だけを包む

- **GIVEN** store actor が internal persist success reply を返す
- **WHEN** effector wrapper が aggregate actor へ通知する
- **THEN** aggregate actor の message 型 `M` は internal reply ではなく `EventSourcedEffectorSignal` を受け取る
- **AND** domain command handler は event-sourced store actor の internal protocol 型を import しない
