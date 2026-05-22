# persistence-effector-typed-api Specification

## Purpose
TBD - created by archiving change persistence-effector-typed-api. Update Purpose after archive.
## Requirements
### Requirement: typed persistence は通常の `Behavior` ベース actor として実装できる

fraktor-rs の typed persistence API は、ユーザー actor に `EventSourcedBehavior` 相当の専用 command handler / event handler DSL を強制してはならない (MUST NOT)。ユーザーは `Behaviors::setup`、`Behaviors::receive_message`、`Behaviors::receive_message_partial`、状態別 handler 関数を使って aggregate actor を実装できなければならない (MUST)。

typed persistence API は `PersistenceEffector::props(config, on_ready)` と低レベルの `PersistenceEffector::from_config(config, on_ready)` を提供し、recovery 完了後に `on_ready(state, effector)` を呼び出して初期 `Behavior<M>` を生成しなければならない (MUST)。

typed persistence API は `modules/persistence-core-typed` / `fraktor-persistence-core-typed-rs` に配置しなければならない (MUST)。classic persistence 基盤は `modules/persistence-core-kernel` / `fraktor-persistence-core-kernel-rs` に配置し、actor runtime 依存は `fraktor-actor-core-kernel-rs` までに留めなければならない (MUST)。`fraktor-utils-core-rs` や no_std 対応の補助 crate への依存はこの actor runtime 境界の制約に含めない。

`fraktor-persistence-core-kernel-rs` は `fraktor-actor-core-typed-rs` に依存してはならない (MUST NOT)。`fraktor-persistence-core-typed-rs` だけが `fraktor-persistence-core-kernel-rs` と `fraktor-actor-core-typed-rs` を合成し、`Behavior`, `Behaviors`, `TypedActorContext`, `StashBuffer`, `TypedProps` と連携しなければならない (MUST)。

#### Scenario: recovery 後に state-specific behavior を開始できる

- **GIVEN** `PersistenceEffectorConfig` に `initial_state` と `apply_event` が設定されている
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
- **WHEN** user が `PersistenceEffector::props(config, on_ready)` で effector aggregate actor を spawn する
- **THEN** returned `TypedProps<M>` は `TypedProps::with_stash_mailbox()` 相当を適用済みである
- **AND** effector implementation は `Behaviors::with_stash` / `StashBuffer<M>` に基づいて stash / unstash する
- **AND** `from_config` を直接使う advanced caller は `TypedProps::from_behavior_factory(...).with_stash_mailbox()` 相当を明示する

#### Scenario: persistence kernel は typed crate に依存しない

- **WHEN** `modules/persistence-core-kernel/Cargo.toml` を確認する
- **THEN** dependency / dev-dependency に `fraktor-actor-core-typed-rs` は存在しない
- **AND** actor runtime dependency としては `fraktor-actor-core-kernel-rs` だけで classic persistence API を提供する

### Requirement: `PersistenceEffector` は event persistence operation を提供する

typed persistence API は `persist_event` と `persist_events` を提供しなければならない (MUST)。これらの operation は event を store actor または mode-specific store に保存し、保存成功後に callback を実行しなければならない (MUST)。

persist callback は operation ごとに一度だけ実行される one-shot callback でなければならない (MUST)。Rust API では `FnOnce` 相当を受け付け、command handler が作った new state を clone せず callback に move できなければならない (MUST)。

persist operation は保存完了前に user command を処理してはならない (MUST NOT)。保存待ち中の user command は `stash_capacity` に従って stash しなければならない (MUST)。保存成功後、callback が返した behavior へ stashed command を戻さなければならない (MUST)。

#### Scenario: 単一 event persist 成功後に callback が実行される

- **GIVEN** aggregate actor が command を処理して event `E1` を生成する
- **WHEN** actor が `effector.persist_event(ctx, E1, callback)` を呼ぶ
- **AND** store actor の reply が `PersistenceEffectorSignal::PersistedEvents([E1])` に変換される
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

### Requirement: recovery は command handler を再実行せず `apply_event` だけで state を復元する

typed persistence effector は recovery 中に保存済み snapshot と event を読み込み、`apply_event(&S, &E) -> S` だけを使って state を復元しなければならない (MUST)。user command handler、domain command method、reply side effect は recovery 中に実行してはならない (MUST NOT)。

#### Scenario: event replay は domain command を再実行しない

- **GIVEN** journal に `Created`, `Deposited` event が保存されている
- **WHEN** actor が再起動する
- **THEN** effector は `initial_state` または snapshot から event を replay する
- **AND** replay では `apply_event` だけを呼び出す
- **AND** command handler の reply / side effect は実行されない

### Requirement: command handler はドメインオブジェクトの新 state を直接利用できる

typed persistence effector を使う aggregate actor は、domain object が返した新 state と event を command handler 内で受け取り、event persistence 成功後に新 state を次の behavior に渡せなければならない (MUST)。

typed persistence effector は、この新 state を `Clone` 可能な値や shared handle に変換することを user callback に要求してはならない (MUST NOT)。内部 waiting behavior は `FnOnce` callback を一度だけ消費し、callback が capture した new state をそのまま次 behavior に渡せなければならない (MUST)。

#### Scenario: ドメイン操作が返した新 state を次 behavior に渡す

- **GIVEN** state `Created { account }` で `DepositCash` command を受け取る
- **WHEN** `account.deposit(amount)` が `Ok((new_account, deposited_event))` を返す
- **THEN** command handler は `deposited_event` を `persist_event` に渡す
- **AND** persist 成功 callback は `new_account` を含む `Created` state の behavior を返す
- **AND** event handler 側で同じ domain validation を二重実行しない

### Requirement: internal store protocol は user message API に露出しない

typed persistence effector は hidden child store actor の internal protocol (`PersistenceStoreCommand` / `PersistenceStoreReply`) を public aggregate API として要求してはならない (MUST NOT)。

typed persistence effector は stable public signal `PersistenceEffectorSignal<S, E>` と `PersistenceEffectorMessageAdapter<S, E, M>` を提供し、actor-private message 型 `M` への wrapping / unwrapping をこの adapter に限定しなければならない (MUST)。

#### Scenario: aggregate message は effector signal だけを包む

- **GIVEN** store actor が internal persist success reply を返す
- **WHEN** effector wrapper が aggregate actor へ通知する
- **THEN** aggregate actor の message 型 `M` は `PersistenceStoreReply` ではなく `PersistenceEffectorSignal` を受け取る
- **AND** domain command handler は store actor の protocol 型を import しない

### Requirement: persistence mode を設定で切り替えられる

typed persistence effector は `PersistenceMode::Persisted`、`PersistenceMode::Ephemeral`、`PersistenceMode::Deferred` を提供しなければならない (MUST)。3 mode は同じ public API、同じ callback ordering、同じ stashing 契約を提供しなければならない (MUST)。

#### Scenario: `Persisted` mode は journal / snapshot store に保存する

- **GIVEN** persistence extension に journal actor と snapshot actor が登録されている
- **WHEN** `PersistenceMode::Persisted` で `persist_event` を呼ぶ
- **THEN** event は configured journal に保存される
- **AND** actor 再起動後に recovery で replay される

#### Scenario: `Ephemeral` mode は actor-system extension store から replay する

- **GIVEN** `PersistenceMode::Ephemeral` の actor が event を保存している
- **WHEN** 同一 actor system 内で同じ persistence id の actor を再作成する
- **THEN** effector は actor-system extension が所有する in-memory snapshot / event から state を復元する
- **AND** external journal plugin は不要である
- **AND** 別 actor system / 別 store scope へ data は漏れない

#### Scenario: `Deferred` mode は storage に書かず callback を実行する

- **GIVEN** `PersistenceMode::Deferred` が設定されている
- **WHEN** `persist_event` を呼ぶ
- **THEN** event は journal に保存されない
- **AND** callback は即時実行される
- **AND** recovery state は常に `initial_state` から開始する

### Requirement: snapshot criteria と retention criteria を提供する

typed persistence effector は snapshot を保存するための `persist_snapshot`、event persist と同時に snapshot criteria を評価する `persist_event_with_snapshot` / `persist_events_with_snapshot` を提供しなければならない (MUST)。

`SnapshotCriteria` は `Never`、`Always`、`Every { number_of_events }`、`Predicate` を表現できなければならない (MUST)。`RetentionCriteria` は snapshot 保存後に保持する snapshot 数を制御できなければならない (SHOULD)。

#### Scenario: event count に基づいて snapshot を保存する

- **GIVEN** `SnapshotCriteria::Every { number_of_events: 2 }` が設定されている
- **WHEN** sequence number 2 の event persist が成功する
- **THEN** effector は callback 完了前に snapshot を保存する

#### Scenario: force snapshot は criteria を無視する

- **GIVEN** `SnapshotCriteria::Never` が設定されている
- **WHEN** `persist_snapshot(snapshot, force = true, callback)` を呼ぶ
- **THEN** snapshot は保存される
- **AND** callback が実行される

#### Scenario: retention criteria は古い snapshot deletion を起動する

- **GIVEN** `RetentionCriteria::snapshot_every(2, keep_snapshots = 2)` が設定されている
- **WHEN** 新しい snapshot 保存が成功する
- **THEN** effector は保持対象外の古い snapshot deletion を store actor に依頼する
- **AND** deletion failure は `PersistenceEffectorSignal::Failed` に変換され、default では fatal persistence failure として扱われる

### Requirement: persistence failure と domain error を分離する

typed persistence effector は domain validation failure と persistence failure を混同してはならない (MUST NOT)。domain validation failure は user command handler が通常の reply と behavior で処理する。persistence failure は infrastructure failure として扱い、default では actor を fatal error で停止しなければならない (MUST)。

#### Scenario: domain validation failure は persistence を呼ばない

- **GIVEN** withdraw command が残高不足になる
- **WHEN** domain object が `Err(InsufficientFunds)` を返す
- **THEN** command handler は failure reply を送る
- **AND** `persist_event` は呼ばれない
- **AND** actor は通常処理を継続する

#### Scenario: journal write failure は success reply を送らない

- **GIVEN** command handler が event を生成して `persist_event` を呼ぶ
- **WHEN** journal write が失敗する
- **THEN** success reply は送られない
- **AND** persistence failure は `ActorError::fatal` として扱われる

### Requirement: no_std core と既存モジュール規約を守る

typed persistence effector 実装は `modules/persistence-core-kernel` と `modules/persistence-core-typed` の no_std 境界を守らなければならない (MUST)。`std::*` を core に導入してはならない (MUST NOT)。新規 public 型は原則 1 型 1 ファイルで配置し、`lib.rs` から明示的に re-export しなければならない (MUST)。

#### Scenario: core module に std dependency を追加しない

- **WHEN** 実装差分を確認する
- **THEN** `modules/persistence-core-kernel/src` と `modules/persistence-core-typed/src` に `std::` import は追加されない
- **AND** in-memory mode は actor-system extension store と `alloc` / 既存 sync primitive だけで実装され、process global singleton を使わない

#### Scenario: public typed persistence API は re-export される

- **WHEN** crate user が `fraktor_persistence_core_typed_rs` を import する
- **THEN** `PersistenceEffector`, `PersistenceEffectorConfig`, `PersistenceEffectorSignal`, `PersistenceEffectorMessageAdapter`, `PersistenceId`, `PersistenceMode`, `SnapshotCriteria`, `RetentionCriteria` を利用できる

### Requirement: typed recovery selection API を提供する

typed persistence API は recovery 実行時にどの snapshot を読み込み、どの event 範囲を replay するかを表す `Recovery` と `SnapshotSelectionCriteria` 相当の public contract を提供しなければならない (MUST)。この recovery selection API は snapshot 書き込みタイミングを表す既存 `SnapshotCriteria<S, E>` と別契約でなければならない (MUST)。

typed recovery selection API は default recovery を表現できなければならない (MUST)。また、snapshot を無効化する recovery、指定 sequence number 以前の snapshot を選択する recovery、指定 timestamp 以前の snapshot を選択する recovery を表現できなければならない (MUST)。

typed persistence config は recovery selection を受け取り、未指定時は既存と同じ default recovery を使わなければならない (MUST)。typed recovery selection は `PersistenceStoreActor` の `Eventsourced::recovery()` を通じて kernel `persistent::Recovery` に変換されなければならない (MUST)。

#### Scenario: default recovery は既存挙動を維持する

- **WHEN** user が recovery selection を明示せずに `PersistenceEffectorConfig` を作成する
- **THEN** config は default recovery を使う
- **AND** recovery は最新利用可能 snapshot とそれ以降の event replay を選択する

#### Scenario: snapshot write criteria と recovery selection は別契約である

- **WHEN** user が `SnapshotCriteria::Every { number_of_events: 10 }` を設定する
- **THEN** その設定は snapshot write timing だけを制御する
- **AND** recovery snapshot selection は typed `Recovery` / typed `SnapshotSelectionCriteria` 側で制御される

#### Scenario: snapshot disabled recovery を表現できる

- **WHEN** user が snapshot を使わない recovery を設定する
- **THEN** recovery は journal event replay だけで state を復元する
- **AND** snapshot write criteria の設定値はこの recovery read decision を上書きしない

#### Scenario: replay limit は kernel recovery へ渡される

- **WHEN** user が replay upper bound と replay max を持つ typed recovery を設定する
- **THEN** `PersistenceStoreActor` は kernel `persistent::Recovery` へ同じ replay bound を渡す
- **AND** `PersistenceContext::start_recovery()` が既存 recovery engine でその bound を使う

### Requirement: typed event adapter と typed snapshot adapter を提供する

typed persistence API は typed event payload を扱う `EventAdapter`、zero / one / many の typed event sequence を表す `EventSeq<E>`、typed state snapshot を変換する `SnapshotAdapter` を提供しなければならない (MUST)。

typed `EventSeq<E>` は empty、single、multiple を表現できなければならない (MUST)。typed `EventAdapter` は event を journal payload へ変換し、journal payload と manifest から typed event sequence へ戻せなければならない (MUST)。typed `SnapshotAdapter` は typed state を snapshot payload へ変換し、snapshot payload と manifest から typed state へ戻せなければならない (MUST)。

typed event adapter API は `PersistenceEffectorConfig` から登録でき、既存 kernel event adapter pipeline と接続できなければならない (MUST)。typed snapshot adapter API は public conversion contract として提供されなければならない (MUST)。ただしこの change は snapshot adapter registry、serializer registry、persisted binary format を追加してはならない (MUST NOT)。

#### Scenario: typed event adapter は one-to-many read adaptation を表現できる

- **GIVEN** journal payload が typed event `E1` と `E2` に展開される
- **WHEN** typed event adapter が journal payload と manifest を読む
- **THEN** adapter は `EventSeq::Multiple([E1, E2])` 相当を返せる

#### Scenario: typed event adapter は manifest を提供できる

- **GIVEN** typed event `E` を journal へ保存する
- **WHEN** typed event adapter が event を journal payload へ変換する
- **THEN** adapter は storage 側へ渡す manifest を返せる

#### Scenario: typed snapshot adapter は state snapshot を round trip できる

- **GIVEN** typed state `S` を snapshot として保存する
- **WHEN** typed snapshot adapter が state を snapshot payload に変換し、その payload と manifest を読み戻す
- **THEN** adapter は typed state `S` を復元できる

#### Scenario: typed snapshot adapter は runtime snapshot store integration を要求しない

- **WHEN** user が typed `SnapshotAdapter<S>` を実装する
- **THEN** adapter は typed snapshot conversion contract として利用できる
- **AND** snapshot store runtime はこの change で snapshot adapter registry を要求しない

### Requirement: typed durable state signal family を提供する

typed persistence API は future `DurableStateBehavior` integration で使う `DurableStateSignal<S>` family を提供しなければならない (MUST)。この signal family は event-sourced `PersistenceEffectorSignal<S, E>` と別型でなければならない (MUST)。

`DurableStateSignal<S>` は recovery completed、recovery failed、state persisted、state deleted、persistence failed を表現できなければならない (MUST)。failure variants は persistence kernel の durable state / persistence error contract と接続できる error payload を持たなければならない (MUST)。

この change は durable state behavior execution、durable state effect builder、reply effect を実装してはならない (MUST NOT)。

#### Scenario: durable state recovery completed を private message に包める

- **GIVEN** typed durable state actor が private message type を持つ
- **WHEN** durable state recovery が完了する
- **THEN** actor は `DurableStateSignal::RecoveryCompleted` 相当を private message に包んで処理できる

#### Scenario: durable state persisted signal は event-sourced signal と混同されない

- **WHEN** durable state update が保存される
- **THEN** durable state API は `DurableStateSignal` の persisted variant を使う
- **AND** event-sourced `PersistenceEffectorSignal::PersistedEvents` は使わない

#### Scenario: durable state API は behavior implementation を要求しない

- **WHEN** user が `DurableStateSignal` を import する
- **THEN** signal type は利用できる
- **AND** `DurableStateBehavior` や durable state `EffectBuilder` はこの change の public API として要求されない

### Requirement: Phase 1 typed parity API は crate root から re-export される

`fraktor-persistence-core-typed-rs` は Phase 1 typed parity API を crate root から re-export しなければならない (MUST)。new public types は `modules/persistence-core-typed/src/` に one public type per file で配置しなければならない (MUST)。

new public API は `no_std` core 境界を維持し、`std::*` に依存してはならない (MUST NOT)。

#### Scenario: crate user は Phase 1 typed parity API を import できる

- **WHEN** crate user が `fraktor_persistence_core_typed_rs` の crate root を import する
- **THEN** typed `Recovery`、typed `SnapshotSelectionCriteria`、typed `EventAdapter`、typed `EventSeq`、typed `SnapshotAdapter`、`DurableStateSignal` を利用できる

#### Scenario: typed crate は no_std を維持する

- **WHEN** implementation diff を確認する
- **THEN** `modules/persistence-core-typed/src/` に `std::` import は追加されない
- **AND** new public types は `alloc` と existing core dependencies だけで構成される

