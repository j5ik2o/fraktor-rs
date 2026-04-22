# pekko-fault-dispatcher-hardening Specification

## Purpose
Pekko `FaultHandling.scala` / `PinnedDispatcher.scala` と同等の fault-path / pinned-dispatcher セマンティクスを fraktor-rs で保証する。具体的には、(1) `ActorCell::report_failure` が Pekko `FaultHandling.scala:221-222` 相当の `isFailed` guard を通して `set_failed(self.pid)` を呼び、初回 failure の perpetrator を記録し、restart / Resume 完了時に `clear_failed()` で状態をクリアする契約、(2) `PinnedDispatcher` が Pekko `PinnedDispatcher.scala:48-59` 相当の 1 actor 1 thread 排他契約を rustdoc で Pekko 行単位対応表として明示する契約、を定める。change `pekko-fault-dispatcher-hardening` (2026-04-23 archive) で確立。

## Requirements
### Requirement: `report_failure` は `isFailed` guard を通して `set_failed(self.pid)` を呼ぶ

`ActorCell::report_failure(&self, error, snapshot)` は、mailbox suspend / children suspend / supervisor 通知を行う前に、`is_failed() == false` の場合に限り `set_failed(self.pid)` を呼ばなければならない (MUST)。既に `is_failed() == true` の場合は `set_failed` を呼ばず (perpetrator 状態を overwrite しない)、残る suspend + supervisor 通知は従来通り実行しなければならない (MUST)。本契約は Pekko `FaultHandling.scala:221-222` の `case _ if !isFailed => setFailed(self)` に意味論的に等価でなければならない (MUST)。

#### Scenario: 初回 report_failure で `FailedInfo::Child(self.pid)` が記録される

- **GIVEN** `ActorCell` が `FailedInfo::None` (= `is_failed() == false` かつ `perpetrator() == None`)
- **WHEN** `report_failure(&error, None)` が呼ばれる
- **THEN** `set_failed(self.pid)` が呼ばれ、state が `FailedInfo::Child(self.pid)` に遷移する
- **AND** `is_failed() == true`、`perpetrator() == Some(self.pid)` になる
- **AND** mailbox は suspend され、supervisor への `FailurePayload` 通知も発火する (既存挙動維持)

#### Scenario: 既に failed 中の report_failure は perpetrator を overwrite しない

- **GIVEN** `ActorCell` が `FailedInfo::Child(existing_pid)` 状態 (`existing_pid != self.pid` も含む)
- **WHEN** 2 回目の `report_failure(&error, None)` が呼ばれる
- **THEN** `set_failed(self.pid)` は **呼ばれない** (is_failed() guard が弾く)
- **AND** `perpetrator() == Some(existing_pid)` のまま不変 (初回記録が保存される)
- **AND** mailbox suspend と supervisor 通知は再度発火する (Pekko も毎回 `sendSystemMessage(Failed)` を送る挙動に準拠)

#### Scenario: `FailedInfo::Fatal` 状態では `set_failed` が downgrade を起こさない

- **GIVEN** `ActorCell` が `FailedInfo::Fatal` (= `set_failed_fatally()` 実行済)
- **WHEN** `report_failure(&error, None)` が呼ばれる
- **THEN** `is_failed() == true` のため内部の `set_failed` 呼び出しは skip される
- **AND** 仮に `set_failed(self.pid)` が直接呼ばれても、既存の `set_failed` 実装 (`actor_cell.rs:448`) の guard により `FailedInfo::Fatal` は保持される (AC-H3 で既に担保)
- **AND** mailbox suspend と supervisor 通知は従来通り発火する

#### Scenario: restart 成功後に `FailedInfo` が `None` にクリアされる

- **GIVEN** `ActorCell` で `report_failure` を経て `FailedInfo::Child(self.pid)` になった cell (`is_failed() == true`)
- **WHEN** supervisor directive Restart を経由して `fault_recreate` → `finish_recreate` が成功完了する
- **THEN** `clear_failed()` が呼ばれ、state が `FailedInfo::None` に戻る (既存配線: `actor_cell.rs:1264`)
- **AND** `is_failed() == false`、`perpetrator() == None` が観測される (次の failure サイクルで新しい perpetrator を記録できる)
- **AND** 本契約は Pekko `FaultHandling.scala:173` `finishCreate` / `:284` `finishRecreate` が restart 成功時に `clearFailed()` を呼ぶ挙動と等価でなければならない

#### Scenario: Resume directive 適用後に `FailedInfo` が `None` にクリアされる

- **GIVEN** `ActorCell` で `report_failure` を経て `FailedInfo::Child(self.pid)` になった cell
- **WHEN** `SystemMessage::Resume` が system_invoke 経由で処理される (supervisor directive Resume 経路)
- **THEN** `SystemMessage::Resume` arm 内で `clear_failed()` が呼ばれ、state が `FailedInfo::None` に戻る
- **AND** `is_failed() == false`、`perpetrator() == None` が観測される
- **AND** children への Resume propagation は従来通り発火する (`resume_children()` 挙動不変)
- **AND** 本契約は Pekko `FaultHandling.scala:150` `faultResume` の `finally if (causedByFailure ne null) clearFailed()` と意味論的に等価でなければならない (fraktor-rs の Resume は causedByFailure を持たないため unconditional クリアで代替する)

---

### Requirement: `PinnedDispatcher` の 1 actor 1 thread 排他契約の rustdoc 明示

`modules/actor-core/src/core/kernel/dispatch/dispatcher/pinned_dispatcher.rs` の `PinnedDispatcher::register_actor` / `unregister_actor` は、Pekko `PinnedDispatcher.scala:48-53` の `if ((actor ne null) && actorCell != actor) throw` 相当の排他契約を既に実装しており、本 change ではその契約を rustdoc で明示する (MUST)。具体的には以下を rustdoc に含めなければならない:

- `register_actor` の 3 分岐 (`None` / `Some(same)` / `Some(other)`) と Pekko `PinnedDispatcher.scala:48-54` の条件 (`actor eq null` / `actor ne null && actorCell eq actor` / `actor ne null && actorCell ne actor`) との行対応。
- `unregister_actor` の owner クリア条件 (`owner == pid` のときのみクリア) が Pekko `PinnedDispatcher.scala:56-59` の `unregister` method 経路と等価であること。Pekko は無条件 `owner = null` だが、fraktor-rs は API 防御的に pid 一致時のみクリアする差分を明記する。
- `&mut self` + 外部 `MessageDispatcherShared` mutex による serialization で race なしに排他が成立する旨 (Pekko の `@volatile` + 外部 attach/detach lock パターン相当)。

#### Scenario: rustdoc が Pekko 参照行を含む

- **WHEN** `pinned_dispatcher.rs` の `register_actor` / `unregister_actor` の rustdoc を読む
- **THEN** `PinnedDispatcher.scala:48-53` への参照が少なくとも 1 箇所含まれる
- **AND** 3 分岐と Pekko 条件の対応表がコメントまたは rustdoc に明示されている

#### Scenario: 既存テストが引き続き pass する

- **GIVEN** `pinned_dispatcher/tests.rs` 内の 5 テスト (`new_normalises_throughput_and_deadline` / `register_actor_sets_owner_and_increments_inhabitants` / `register_actor_rejects_second_owner` / `register_actor_allows_same_actor_to_reattach` / `unregister_actor_clears_owner_after_detach` / `detach_then_new_owner_can_register`)
- **WHEN** 本 change 適用後に `cargo test` を実行する
- **THEN** 6 テスト全てが pass する (rustdoc 追加のみで挙動不変)
- **AND** 本 change で `#[ignore]` 新規付与なし
