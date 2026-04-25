## Phase 1: 準備と境界確認

- [ ] 1.1 `modules/persistence-core/src/core` の existing classic persistence flow (`PersistentActor`, `PersistenceContext`, `PersistentActorAdapter`) を再確認する
- [ ] 1.2 `modules/actor-core/src/core/typed` の `Behavior`, `TypedActorContext`, `message_adapter`, `stash`, `unstash_all`, `TypedProps` の制約を確認する
- [ ] 1.3 `pekko-persistence-effector` の `PersistenceEffector`, `PersistenceEffectorConfig`, `PersistenceMode`, `SnapshotCriteria`, `RetentionCriteria` を参照し、Rust に移植する概念と移植しない概念を確定する
- [ ] 1.4 `docs/gap-analysis/persistence-gap-analysis.md` の typed write-side gap を読み、実装完了後に更新する箇所を特定する

## Phase 2: public typed module skeleton

- [ ] 2.1 `modules/persistence-core/src/core/typed.rs` を追加し、`core.rs` から公開する
- [ ] 2.2 `PersistenceId` を追加する (`of_unique_id`, `of_entity_id`, `as_str`)
- [ ] 2.3 `PersistenceMode` を追加する (`Persisted`, `Ephemeral`, `Deferred`)
- [ ] 2.4 `BackoffConfig` を追加する
- [ ] 2.5 `SnapshotCriteria<S, E>` を追加する (`never`, `always`, `every`, `predicate`)
- [ ] 2.6 `RetentionCriteria` を追加する (`none`, `snapshot_every`)
- [ ] 2.7 rustdoc を日本語で追加し、missing_docs を満たす

## Phase 3: config / converter

- [ ] 3.1 `PersistenceEffectorConfig<S, E, M>` を追加する
- [ ] 3.2 `apply_event: Fn(&S, &E) -> S` を config に保持する
- [ ] 3.3 `PersistenceEffectorMessageConverter<S, E, M>` を追加する
- [ ] 3.4 builder-style `with_persistence_mode`, `with_stash_capacity`, `with_snapshot_criteria`, `with_retention_criteria`, `with_backoff_config`, `with_message_converter` を追加する
- [ ] 3.5 config validation を追加する (`stash_capacity > 0`, snapshot interval > 0, retention > 0)

## Phase 4: internal store protocol

- [ ] 4.1 `typed/internal/persistence_store_command.rs` を追加する
- [ ] 4.2 `typed/internal/persistence_store_reply.rs` を追加する
- [ ] 4.3 `typed/internal/persistence_store_actor.rs` を追加する
- [ ] 4.4 store actor は existing classic persistence primitives を使って recovery / persist / snapshot / delete を実行する
- [ ] 4.5 recovery 完了時に `RecoveryCompleted { state, sequence_nr }` を返す
- [ ] 4.6 persist failure / snapshot failure は `Failed { error }` として返す

## Phase 5: `PersistenceEffector` behavior builder

- [ ] 5.1 `PersistenceEffector<S, E, M>` を追加する
- [ ] 5.2 `PersistenceEffector::from_config(config, on_ready) -> Behavior<M>` を追加する
- [ ] 5.3 `Persisted` mode で store child actor を起動し、recovery 完了まで user command を stash する
- [ ] 5.4 recovery 完了後、`on_ready(state, effector)` が返した behavior へ unstash する
- [ ] 5.5 store reply を `message_converter` 経由で aggregate message `M` に包む

## Phase 6: persist / snapshot operations

- [ ] 6.1 `persist_event` を追加し、store reply 待ち behavior を返す
- [ ] 6.2 `persist_events` を追加し、複数 event を 1 batch として扱う
- [ ] 6.3 `persist_snapshot(snapshot, force, callback)` を追加する
- [ ] 6.4 `persist_event_with_snapshot` / `persist_events_with_snapshot` を追加し、snapshot criteria を評価する
- [ ] 6.5 persist 中の user command は `stash_capacity` に従って stash する
- [ ] 6.6 persist 成功後 callback を実行し、その behavior へ `unstash_all` する
- [ ] 6.7 persistence failure は default で `ActorError::fatal` に変換する

## Phase 7: mode-specific behavior

- [ ] 7.1 `Ephemeral` mode 用 in-memory event / snapshot store を追加する
- [ ] 7.2 `Ephemeral` recovery は latest snapshot + subsequent events を replay する
- [ ] 7.3 `Deferred` mode は storage へ書かず callback を即時実行する
- [ ] 7.4 3 mode で public API と callback ordering が一致することをテストする

## Phase 8: snapshot / retention

- [ ] 8.1 `SnapshotCriteria::every` が sequence number に基づいて snapshot を保存することを実装する
- [ ] 8.2 `SnapshotCriteria::predicate` が event / state / sequence number を受け取ることを実装する
- [ ] 8.3 `RetentionCriteria::snapshot_every` に基づいて古い snapshot deletion command を store actor へ送る
- [ ] 8.4 delete snapshot failure の扱いを fatal / warn のどちらにするか実装前に確定し、仕様へ反映する

## Phase 9: examples / integration tests

- [ ] 9.1 typed bank account aggregate example を追加する
- [ ] 9.2 `NotCreated` / `Created` の state-specific behavior 分割例を追加する
- [ ] 9.3 domain object が `Result<NewState, Event>` を返し、command handler が新 state を次 behavior に渡す例を追加する
- [ ] 9.4 `Persisted` + `InMemoryJournal` / `InMemorySnapshotStore` integration test を追加する
- [ ] 9.5 `Ephemeral` mode unit test を追加する
- [ ] 9.6 `Deferred` mode unit test を追加する
- [ ] 9.7 persist 中 stashing / recovery 中 stashing のテストを追加する
- [ ] 9.8 persistence failure と domain validation failure が混ざらないことをテストする

## Phase 10: docs / gap-analysis / verification

- [ ] 10.1 `docs/gap-analysis/persistence-gap-analysis.md` を更新し、typed write-side 方針を effector-first として反映する
- [ ] 10.2 `README.ja.md` または persistence docs に typed persistence effector の短い利用例を追加する
- [ ] 10.3 `rtk cargo test -p fraktor-persistence-core-rs` を実行する
- [ ] 10.4 `./scripts/ci-check.sh ai dylint` を実行する
- [ ] 10.5 最終確認として `./scripts/ci-check.sh ai all` を実行する
