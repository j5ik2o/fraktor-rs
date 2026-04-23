## ADDED Requirements

### Requirement: `watch` / `watch_with` は同一 target への異種登録を error で拒否する

`ActorContext::watch(&mut self, target)` および `ActorContext::watch_with(&mut self, target, message)` は、同一 target に対して既に User watch が登録されている場合、以下の分岐に従って結果を返さなければならない (MUST):

- **`watch(t)` を呼び、既存登録が plain `watch(t)`**: `Ok(())` を返し、副作用は起こさない (Pekko parity: `None == None` idempotent)。
- **`watch(t)` を呼び、既存登録が `watch_with(t, _)`**: `Err(WatchRegistrationError::Duplicate { target: t.pid(), conflict: WatchConflict::WatchWithThenPlain })` を返し、既存 entry を保持する。
- **`watch_with(t, m_new)` を呼び、既存登録が plain `watch(t)`**: `Err(WatchRegistrationError::Duplicate { target: t.pid(), conflict: WatchConflict::PlainThenWatchWith })` を返し、既存 entry を保持する。また `m_new` は drop されるため caller は message 復元を期待してはならない。
- **`watch_with(t, m_new)` を呼び、既存登録が `watch_with(t, m_prev)`**: `Err(WatchRegistrationError::Duplicate { target: t.pid(), conflict: WatchConflict::WatchWithThenWatchWith })` を返し、既存 entry を保持する。`m_new` は drop される。Pekko は `m_prev == m_new` の場合のみ許容するが、本仕様は `AnyMessage` の同値判定不能性により conservative に全拒否する (設計 Decision 5)。
- **target が self の場合**: 従来通り `Ok(())` を返し、watching 登録も副作用も行わない。
- **target が未登録 (`watching` に無い)場合**: 従来通り `Watch` system message を送信し、`watching` / `watch_with_messages` に登録する。送信失敗 (`SendError` 派生) は `WatchRegistrationError::Send(..)` に wrap して返す。

本契約は Pekko `DeathWatch.scala:36-66, 126-132` の `watch` / `watchWith` + `checkWatchingSame` と意味論的に等価でなければならない (MUST、ただし Decision 5 の conservative 拒否は明示的 divergence として許容)。

#### Scenario: plain watch 後の plain watch は no-op

- **GIVEN** actor A が actor B を `watch(B)` 済 (User watch 登録、watch_with_message なし)
- **WHEN** A が再度 `watch(B)` を呼ぶ
- **THEN** `Ok(())` が返る
- **AND** `watching` エントリは 1 件のまま (`watching_contains_pid(B.pid()) == true`)
- **AND** `watch_with_messages` に新規 entry は追加されない
- **AND** `SystemMessage::Watch` の 2 回目送信は発生しない

#### Scenario: plain watch 後の watch_with は拒否

- **GIVEN** actor A が actor B を `watch(B)` 済
- **WHEN** A が `watch_with(B, msg_new)` を呼ぶ
- **THEN** `Err(WatchRegistrationError::Duplicate { target: B.pid(), conflict: WatchConflict::PlainThenWatchWith })` が返る
- **AND** `watch_with_messages` に新規 entry は追加されない
- **AND** `msg_new` は drop される (caller は復元不可)
- **AND** 既存 plain watch は保持されたまま

#### Scenario: watch_with 後の plain watch は拒否

- **GIVEN** actor A が actor B を `watch_with(B, msg_prev)` 済
- **WHEN** A が `watch(B)` を呼ぶ
- **THEN** `Err(WatchRegistrationError::Duplicate { target: B.pid(), conflict: WatchConflict::WatchWithThenPlain })` が返る
- **AND** `watch_with_messages` に `(B.pid(), msg_prev)` が保持されたまま
- **AND** target が terminate したら custom message `msg_prev` が配送される (既存 watch_with 契約維持)

#### Scenario: watch_with 後の watch_with は常に拒否 (conservative)

- **GIVEN** actor A が actor B を `watch_with(B, msg_prev)` 済
- **WHEN** A が `watch_with(B, msg_new)` を呼ぶ (msg_prev と msg_new が「概念的に同一」でも可)
- **THEN** `Err(WatchRegistrationError::Duplicate { target: B.pid(), conflict: WatchConflict::WatchWithThenWatchWith })` が返る
- **AND** `watch_with_messages` に `(B.pid(), msg_prev)` が保持されたまま (overwrite しない)
- **AND** `msg_new` は drop される
- **NOTE** Pekko との divergence: Pekko は `Some(msg_prev) == Some(msg_new)` の場合は no-op。fraktor-rs は `AnyMessage` 同値判定不能のため常に拒否。caller は意図的に再設定したければ先に `unwatch(B)` を呼ぶ必要がある。

#### Scenario: unwatch 後の watch_with は成功する

- **GIVEN** actor A が actor B を `watch_with(B, msg_prev)` 済
- **WHEN** A が `unwatch(B)` を呼び、続けて `watch_with(B, msg_new)` を呼ぶ
- **THEN** `unwatch` が `Ok(())` を返し、`watch_with_messages` から `(B.pid(), msg_prev)` が除去される
- **AND** `watch_with(B, msg_new)` が `Ok(())` を返す
- **AND** `watch_with_messages` に `(B.pid(), msg_new)` が登録される (新規扱い)
- **AND** target B terminate 時に msg_new が配送される

#### Scenario: target が self の場合は無視される (従来挙動)

- **GIVEN** actor A
- **WHEN** A が `watch(A_ref)` または `watch_with(A_ref, msg)` を呼ぶ (self を target に指定)
- **THEN** `Ok(())` が返る
- **AND** `watching` / `watch_with_messages` に entry は追加されない
- **AND** `SystemMessage::Watch` は送信されない

#### Scenario: 未登録 target への watch_with は従来通り登録される

- **GIVEN** actor A が actor B を一度も watch していない (`watching_contains_pid(B.pid()) == false`)
- **WHEN** A が `watch_with(B, msg)` を呼ぶ
- **THEN** `Ok(())` が返る
- **AND** `watch_with_messages` に `(B.pid(), msg)` が追加される
- **AND** `watching` に User kind の entry が追加される
- **AND** `SystemMessage::Watch` が B に送信される
- **AND** 送信失敗時は既存挙動通り rollback (`watch_with_messages` から entry 除去) + `WatchRegistrationError::Send(..)` を返す

---

### Requirement: `ActorCell` は User watch 登録種別を query できる

`ActorCell` は `pub(crate) fn watch_registration_kind(&self, target: Pid) -> WatchRegistrationKind` 相当の query accessor を提供し、`watching` と `watch_with_messages` を合成して User watch の登録状態を以下の 3 値のいずれかで返さなければならない (MUST):

- `WatchRegistrationKind::None`: 対象 target に対して User watch が未登録
- `WatchRegistrationKind::Plain`: `watching` に User kind で登録済、かつ `watch_with_messages` に entry なし
- `WatchRegistrationKind::WithMessage`: `watching` に User kind で登録済、かつ `watch_with_messages` に entry あり

Supervision kind (`WatchKind::Supervision`) の登録は本 query の判定に含めてはならない (MUST NOT)。Supervision watch は親 → 子の internal supervision path であり、user-level の `watch` / `watch_with` 重複検出の対象外のため。

#### Scenario: 未登録 target は None を返す

- **GIVEN** `ActorCell` が target pid P を一度も watch していない
- **WHEN** `cell.watch_registration_kind(P)` を呼ぶ
- **THEN** `WatchRegistrationKind::None` が返る

#### Scenario: plain watch 済 target は Plain を返す

- **GIVEN** `ActorCell` が `register_watching(P, WatchKind::User)` を実行済
- **AND** `watch_with_messages` に P の entry なし
- **WHEN** `cell.watch_registration_kind(P)` を呼ぶ
- **THEN** `WatchRegistrationKind::Plain` が返る

#### Scenario: watch_with 済 target は WithMessage を返す

- **GIVEN** `ActorCell` が `register_watching(P, WatchKind::User)` + `register_watch_with(P, msg)` を実行済
- **WHEN** `cell.watch_registration_kind(P)` を呼ぶ
- **THEN** `WatchRegistrationKind::WithMessage` が返る

#### Scenario: 親が子を spawn しただけの状態は None 扱い

- **GIVEN** actor A が actor B を `spawn_child(props)` で子として生成 (kernel が `WatchKind::Supervision` で registered を自動登録するが、User watch は未登録)
- **WHEN** `cell.watch_registration_kind(B.pid())` を呼ぶ
- **THEN** `WatchRegistrationKind::None` が返る (supervision 対象は user-level duplicate check 対象外)

#### Scenario: spawn_child_watched で User + Supervision が共存する場合は Plain

- **GIVEN** actor A が `spawn_child_watched(props)` で子 B を生成 (Supervision watch + User watch が両方 register される)
- **AND** `watch_with_messages` に B の entry なし
- **WHEN** `cell.watch_registration_kind(B.pid())` を呼ぶ
- **THEN** `WatchRegistrationKind::Plain` が返る (User watch 存在を優先、Supervision entry は判定に影響しない)
