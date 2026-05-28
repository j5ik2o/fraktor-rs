## ADDED Requirements

### Requirement: typed State Sourcing は通常の `Behavior` ベース actor として実装できる

fraktor-rs の typed state-sourced persistence API は、ユーザー actor に `DurableStateBehavior` 相当の専用 command handler DSL を強制してはならない (MUST NOT)。ユーザーは `Behaviors::setup`、`Behaviors::receive_message`、`Behaviors::receive_message_partial`、状態別 handler 関数を使って aggregate actor を実装できなければならない (MUST)。

typed state-sourced persistence API は `StateSourcedEffector::props(config, on_ready)` と低レベルの `StateSourcedEffector::from_config(config, on_ready)` を提供し、recovery 完了後に recovered state と initialized effector を渡して初期 `Behavior<M>` を生成しなければならない (MUST)。

`StateSourcedEffectorConfig` は event type `E` と `apply_event(&S, &E) -> S` を要求してはならない (MUST NOT)。State Sourcing の recovery は durable state store の latest object と revision から開始しなければならない (MUST)。

#### Scenario: durable state recovery 後に state-specific behavior を開始できる

- **GIVEN** durable state store に state `S1` と revision `3` が保存されている
- **WHEN** actor が `StateSourcedEffector::props(config, on_ready)` で起動する
- **THEN** effector は durable state store から `S1` と revision `3` を読み込む
- **AND** `on_ready(Some(S1), effector)` が呼び出される
- **AND** ユーザーは recovered state の有無に応じて別々の `Behavior<M>` を返せる

#### Scenario: empty durable state は command handler に委ねられる

- **GIVEN** durable state store に対象 persistence id の object が存在しない
- **WHEN** actor が recovery を完了する
- **THEN** `on_ready(None, effector)` が呼び出される
- **AND** effector は internal revision を `0` として保持する
- **AND** ユーザーは empty state 用の `Behavior<M>` を返せる

#### Scenario: recovery 中の user command は stash される

- **GIVEN** actor が durable state recovery 中である
- **WHEN** user command が mailbox に届く
- **THEN** effector wrapper はその command を stash する
- **AND** recovery 完了後に `on_ready` が返した behavior へ unstash する

### Requirement: `StateSourcedEffector` は state persistence operation を提供する

typed state-sourced persistence API は `persist_state` を提供しなければならない (MUST)。`persist_state` は durable state store の `upsert_object` を現在 revision を expected revision として呼び、保存成功後に callback を実行しなければならない (MUST)。

persist callback は operation ごとに一度だけ実行される one-shot callback でなければならない (MUST)。callback は保存済み state と保存後 revision を受け取り、次の `Behavior<M>` を返さなければならない (MUST)。

persist operation は保存完了前に user command を処理してはならない (MUST NOT)。保存待ち中の user command は `stash_capacity` に従って stash しなければならない (MUST)。保存成功後、callback が返した behavior へ stashed command を戻さなければならない (MUST)。

#### Scenario: state persist 成功後に callback が実行される

- **GIVEN** actor が revision `3` の state から command を処理して new state `S2` を生成する
- **WHEN** actor が `effector.persist_state(ctx, S2, callback)` を呼ぶ
- **THEN** state store は `upsert_object(persistence_id, 3, S2, tag)` を呼ばれる
- **AND** 保存成功後に effector の current revision は `4` になる
- **AND** callback は `S2` と revision `4` を受け取って実行される
- **AND** callback が返した behavior が次の active behavior になる

#### Scenario: persist 中の command は保存成功まで処理されない

- **GIVEN** `persist_state` の effector signal を待っている
- **WHEN** 別の user command が届く
- **THEN** effector wrapper は command を stash する
- **AND** persist 成功 callback 完了後に unstash する

#### Scenario: revision mismatch は persistence failure として扱われる

- **GIVEN** effector の current revision が `3` である
- **WHEN** durable state store が `DurableStateError::UpsertRevision` を返す
- **THEN** success callback は実行されない
- **AND** failure は state-sourced effector signal に変換される
- **AND** default では actor は fatal persistence failure として停止する

### Requirement: `StateSourcedEffector` は state delete operation を提供する

typed state-sourced persistence API は `delete_state` を提供しなければならない (MUST)。`delete_state` は durable state store の `delete_object` を現在 revision を expected revision として呼び、削除成功後に callback を実行しなければならない (MUST)。

delete callback は operation ごとに一度だけ実行される one-shot callback でなければならない (MUST)。callback は削除後 revision を受け取り、次の `Behavior<M>` を返さなければならない (MUST)。

#### Scenario: state delete 成功後に callback が実行される

- **GIVEN** actor が revision `4` の state を保持している
- **WHEN** actor が `effector.delete_state(ctx, callback)` を呼ぶ
- **THEN** state store は `delete_object(persistence_id, 4)` を呼ばれる
- **AND** 削除成功後に callback は deleted revision を受け取って実行される
- **AND** callback が返した behavior が次の active behavior になる

#### Scenario: delete revision mismatch は persistence failure として扱われる

- **GIVEN** effector の current revision が `4` である
- **WHEN** durable state store が `DurableStateError::DeleteRevision` を返す
- **THEN** success callback は実行されない
- **AND** failure は state-sourced effector signal に変換される
- **AND** default では actor は fatal persistence failure として停止する

### Requirement: State Sourcing signal は user message API にだけ露出する

typed state-sourced persistence API は stable public signal `StateSourcedEffectorSignal<S>` と `StateSourcedEffectorMessageAdapter<S, M>` を提供しなければならない (MUST)。actor-private message 型 `M` への wrapping / unwrapping はこの adapter に限定しなければならない (MUST)。

state-sourced internal store protocol は user aggregate API に露出してはならない (MUST NOT)。signal construction に必要な auth marker は external crate から偽造できてはならない (MUST NOT)。

#### Scenario: aggregate message は state-sourced effector signal だけを包む

- **GIVEN** durable state store actor が internal persist success reply を返す
- **WHEN** effector wrapper が aggregate actor へ通知する
- **THEN** aggregate actor の message 型 `M` は internal reply ではなく `StateSourcedEffectorSignal` を受け取る
- **AND** domain command handler は state-sourced store actor の internal protocol 型を import しない

#### Scenario: external crate は trusted signal を forge できない

- **WHEN** external crate が `StateSourcedEffectorSignal::StatePersisted` を直接構築しようとする
- **THEN** auth marker を作れないため trusted signal を構築できない
- **AND** external crate は受信済み signal を own private message に wrap することだけができる

### Requirement: State Sourcing Effector は no_std core 境界と durable state store contract を守る

typed state-sourced persistence API は `modules/persistence-core-typed` / `fraktor-persistence-core-typed-rs` に配置しなければならない (MUST)。durable state store contract は `modules/persistence-core-kernel` / `fraktor-persistence-core-kernel-rs` に配置し、typed crate に依存してはならない (MUST NOT)。

`StateSourcedEffector` は kernel の `DurableStateStore` / `GetObjectResult` / `DurableStateError` 契約を利用しなければならない (MUST)。core modules に `std::*` import、filesystem dependency、process global singleton を追加してはならない (MUST NOT)。

#### Scenario: state-sourced API は crate root から re-export される

- **WHEN** crate user が `fraktor_persistence_core_typed_rs` を import する
- **THEN** `StateSourcedEffector`, `StateSourcedEffectorConfig`, `StateSourcedEffectorSignal`, `StateSourcedEffectorMessageAdapter` を利用できる
- **AND** `DurableStateStore` contract は kernel crate 側に残る

#### Scenario: core module に std dependency を追加しない

- **WHEN** 実装差分を確認する
- **THEN** `modules/persistence-core-kernel/src` と `modules/persistence-core-typed/src` に `std::` import は追加されない
- **AND** state-sourced tests は既存 no_std 境界を壊さない
