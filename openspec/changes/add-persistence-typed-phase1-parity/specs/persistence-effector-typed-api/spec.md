## ADDED Requirements

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
