## Phase 1: 準備と境界確認

- [x] 1.1 現行 `modules/persistence-core/src` の existing classic persistence flow (`PersistentActor`, `PersistenceContext`, `PersistentActorAdapter`) を再確認する
- [x] 1.2 `modules/actor-core-typed/src` の `Behavior`, `TypedActorContext`, `message_adapter`, `StashBuffer`, `unstash_all`, `TypedProps::with_stash_mailbox` の制約を確認する
- [x] 1.3 `pekko-persistence-effector` の `PersistenceEffector`, `PersistenceEffectorConfig`, `PersistenceMode`, `SnapshotCriteria`, `RetentionCriteria` を参照し、Rust に移植する概念と移植しない概念を確定する
- [x] 1.4 `docs/gap-analysis/persistence-gap-analysis.md` の typed write-side gap を読み、実装完了後に更新する箇所を特定する

## Phase 2: persistence kernel crate rename

- [x] 2.1 `modules/persistence-core/` を `modules/persistence-core-kernel/` へ rename する
- [x] 2.2 package 名を `fraktor-persistence-core-kernel-rs` に更新する
- [x] 2.3 workspace root `Cargo.toml` の member / workspace dependency を `persistence-core-kernel` へ更新する
- [x] 2.4 downstream `fraktor-persistence-core-rs` references を `fraktor-persistence-core-kernel-rs` へ更新する
- [x] 2.5 `modules/persistence-core-kernel/Cargo.toml` が `fraktor-actor-core-typed-rs` に依存せず、actor runtime dependency を `fraktor-actor-core-kernel-rs` に留めていることを確認する

## Phase 3: public typed module skeleton

- [x] 3.1 `modules/persistence-core-typed/` を追加し、workspace member に登録する
- [x] 3.2 `modules/persistence-core-typed/Cargo.toml` に `fraktor-persistence-core-kernel-rs` と `fraktor-actor-core-typed-rs` dependency を追加する
- [x] 3.3 `modules/persistence-core-typed/src/lib.rs` を追加し、typed public API を re-export する
- [x] 3.4 `PersistenceId` を追加する (`of_unique_id`, `of_entity_id`, `as_str`)
- [x] 3.5 `PersistenceMode` を追加する (`Persisted`, `Ephemeral`, `Deferred`)
- [x] 3.6 `BackoffConfig` を追加する
- [x] 3.7 `PersistenceEffectorSignal<S, E>` を追加する
- [x] 3.8 `SnapshotCriteria<S, E>` を追加する (`never`, `always`, `every`, `predicate`)
- [x] 3.9 `RetentionCriteria` を追加する (`none`, `snapshot_every`)
- [x] 3.10 rustdoc を日本語で追加し、missing_docs を満たす

## Phase 4: config / adapter

- [x] 4.1 `PersistenceEffectorConfig<S, E, M>` を追加する
- [x] 4.2 `apply_event: Fn(&S, &E) -> S` を config に保持する
- [x] 4.3 `PersistenceEffectorMessageAdapter<S, E, M>` を追加し、`PersistenceEffectorSignal` と actor-private message `M` の wrap / unwrap を定義する
- [x] 4.4 builder-style `with_persistence_mode`, `with_stash_capacity`, `with_snapshot_criteria`, `with_retention_criteria`, `with_backoff_config`, `with_message_adapter` を追加する
- [x] 4.5 config validation を追加する (`stash_capacity > 0`, snapshot interval > 0, retention > 0)

## Phase 5: internal store protocol

- [x] 5.1 `internal/persistence_store_command.rs` を追加する
- [x] 5.2 `internal/persistence_store_reply.rs` を追加する
- [x] 5.3 `internal/persistence_store_actor.rs` を追加する
- [x] 5.4 store actor は `fraktor-persistence-core-kernel-rs` の classic persistence primitives を使って recovery / persist / snapshot / delete を実行する
- [x] 5.5 recovery 完了時に `RecoveryCompleted { state, sequence_nr }` を返す
- [x] 5.6 persist failure / snapshot failure は `Failed { error }` として返す

## Phase 6: `PersistenceEffector` behavior builder

- [x] 6.1 `PersistenceEffector<S, E, M>` を追加する
- [x] 6.2 `PersistenceEffector::from_config(config, on_ready) -> Behavior<M>` と `PersistenceEffector::props(config, on_ready) -> TypedProps<M>` を追加する
- [x] 6.3 `Persisted` mode で store child actor を起動し、recovery 完了まで user command を stash する
- [x] 6.4 recovery 完了後、`on_ready(state, effector)` が返した behavior へ unstash する
- [x] 6.5 internal store reply を `PersistenceEffectorSignal` へ変換し、`message_adapter` 経由で aggregate message `M` に包む

## Phase 7: persist / snapshot operations

- [x] 7.1 `persist_event` を追加し、effector signal 待ち behavior を返す
- [x] 7.2 `persist_events` を追加し、複数 event を 1 batch として扱う
- [x] 7.3 `persist_snapshot(snapshot, force, callback)` を追加する
- [x] 7.4 `persist_event_with_snapshot` / `persist_events_with_snapshot` を追加し、snapshot criteria を評価する
- [x] 7.5 persist 中の user command は `StashBuffer<M>` と `stash_capacity` に従って stash する
- [x] 7.6 persist 成功後 `FnOnce` callback を一度だけ実行し、その behavior へ `unstash_all` する
- [x] 7.7 persistence failure は default で `ActorError::fatal` に変換する

## Phase 8: mode-specific behavior

- [x] 8.1 `Ephemeral` mode 用 internal actor-system extension store を追加し、process global singleton を使わない
- [x] 8.2 `Ephemeral` recovery は latest snapshot + subsequent events を replay する
- [x] 8.3 `Deferred` mode は storage へ書かず callback を即時実行する
- [x] 8.4 3 mode で public API と callback ordering が一致することをテストする

## Phase 9: snapshot / retention

- [x] 9.1 `SnapshotCriteria::every` が sequence number に基づいて snapshot を保存することを実装する
- [x] 9.2 `SnapshotCriteria::predicate` が event / state / sequence number を受け取ることを実装する
- [x] 9.3 `RetentionCriteria::snapshot_every` に基づいて古い snapshot deletion command を store actor へ送る
- [x] 9.4 delete snapshot failure の扱いを fatal / warn のどちらにするか実装前に確定し、仕様へ反映する

## Phase 10: showcases/std / integration tests

- [x] 10.1 `showcases/std/typed/persistence_effector/` に typed bank account aggregate showcase を追加する (`modules/**/examples` には置かない)
- [x] 10.2 showcase 内で `NotCreated` / `Created` の state-specific behavior 分割例を追加する
- [x] 10.3 showcase 内で domain object が `Result<NewState, Event>` を返し、command handler が new state を callback に move して次 behavior に渡す例を追加する
- [x] 10.4 showcase 内で actor-private message に `PersistenceEffectorSignal` を包む例を追加し、domain command API と分離する
- [x] 10.5 `Persisted` + `InMemoryJournal` / `InMemorySnapshotStore` integration test を追加する
- [x] 10.6 `Ephemeral` mode unit test を追加する
- [x] 10.7 `Deferred` mode unit test を追加する
- [x] 10.8 persist 中 stashing / recovery 中 stashing のテストを追加する
- [x] 10.9 persistence failure と domain validation failure が混ざらないことをテストする
- [x] 10.10 showcases / tests では `PersistenceEffector::props(config, on_ready)` を使い、`from_config` 直接利用ケースだけ `TypedProps::with_stash_mailbox()` を明示する

## Phase 11: docs / gap-analysis / verification

- [x] 11.1 `docs/gap-analysis/persistence-gap-analysis.md` を更新し、`persistence-core-kernel` / `persistence-core-typed` 境界と typed write-side 方針を effector-first として反映する
- [x] 11.2 `README.ja.md` または persistence docs に typed persistence effector の短い利用例を追加する
- [x] 11.3 `cargo test -p fraktor-persistence-core-kernel-rs` を実行する
- [x] 11.4 `cargo test -p fraktor-persistence-core-typed-rs` を実行する
- [x] 11.5 `./scripts/ci-check.sh ai dylint` を実行する
- [x] 11.6 最終確認として `./scripts/ci-check.sh ai all` を実行する
