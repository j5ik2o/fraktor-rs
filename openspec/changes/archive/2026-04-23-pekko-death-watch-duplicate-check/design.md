## Context

Pekko `dungeon/DeathWatch.scala:36-66, 126-132` の 3 つの挙動:

1. **`watch(a)`**: `watching` に a が無ければ `Watch` system message を送り、`watching` に `a → None` を登録。既に存在すれば `checkWatchingSame(a, None)` を呼び、既存 message が `Some(_)` なら `IllegalStateException`。
2. **`watchWith(a, msg)`**: 未登録なら `Watch` 送信 + `watching` に `a → Some(msg)` を登録。既に存在すれば `checkWatchingSame(a, Some(msg))` で厳密比較し、不一致なら `IllegalStateException`。
3. **`unwatch(a)`**: `watching -= a` + `Unwatch` を送信 + `terminatedQueued -= a`。

fraktor-rs 現状 (`actor_context.rs:316-402`, `actor_cell.rs:1145-1166`):

- `register_watching` と `register_watch_with` は **完全に無条件 overwrite**。既存 entry があっても validate しない。
- `watching` (User / Supervision の `WatchKind` 付き) と `watch_with_messages` (`Vec<(Pid, AnyMessage)>`) は分離データ構造。
- Pekko の `Option[Any]` 単一 map と等価な論理関数は「`watching.contains(target) && watch_with_messages.find(target).map(kind)`」で導出可能。
- `watch_with_messages` の既存 retain-and-push は silent overwrite バグの温床であり、これが本 change の主な修正対象。

`SendError` は常に `AnyMessage` を保有するため、plain `watch()` 重複検出の表現媒体として不自然。plain watch では user message が存在しない。

`AnyMessage` には `PartialEq` が無く、`Box<dyn Any + Send + 'static>` 相当のため dynamic dispatch 越しの value equality は不能。Pekko は Scala `Any` の `!=` (case class の structural equality) に依存しており、この差分は言語制約として avoid-able ではない。

## Goals / Non-Goals

**Goals:**
- Pekko `checkWatchingSame` の意味論等価な契約を kernel レベルで実装する (local 重複検出)。
- `watch` / `watch_with` の silent overwrite を廃止し、衝突は明示的な error として呼び出し元に返す。
- conservative 戦略: Pekko の「同一 message なら許容」は実装不能のため、「watch_with 後の watch_with は常に拒否」へ倒す。Pekko の許容集合の真部分集合にすることで false-positive を防ぐ。
- gap-analysis AC-M4 を 2 責務に分割し、本 change が閉塞する範囲を明確化する。

**Non-Goals:**
- `maintainAddressTerminatedSubscription` 相当の EventStream 購読 — remote/cluster 基盤が整備されていないため別 change で対応 (新規ギャップ `AC-M4b` として記録)。
- `AnyMessage` に `PartialEq` を追加する変更 — dyn trait 制約下で value equality を導入するには payload に `Message + PartialEq` bound を要求する必要があり、public API が侵食される。本 change では避ける。
- 既存 `watching` / `watch_with_messages` データ構造の統合 (Pekko 風 `Map<Pid, Option<AnyMessage>>` 化) — 内部リファクタリングで挙動不変のため別 PR 推奨。
- `post_restart` や `unwatchWatchedActors` の契約変更 — AL-H1 / AC-H3 で既に閉塞済み。

## Decisions

### Decision 1: 新規 error 型 `WatchRegistrationError` を導入する

- **選択**: `modules/actor-core/src/core/kernel/actor/error/watch_registration_error.rs` を新設し、以下の variant を定義:
  ```rust
  pub enum WatchRegistrationError {
    /// Underlying send failure when dispatching Watch system message.
    Send(SendError),
    /// Duplicate registration with conflicting watch-with state.
    Duplicate { target: Pid, conflict: WatchConflict },
  }

  pub enum WatchConflict {
    /// Existing plain watch; caller tried watch_with.
    PlainThenWatchWith,
    /// Existing watch_with; caller tried plain watch.
    WatchWithThenPlain,
    /// Existing watch_with; caller tried another watch_with (message equality undecidable).
    WatchWithThenWatchWith,
  }
  ```
- **Rationale**: `SendError` は常に `AnyMessage` を保持するが、plain `watch()` 重複では user message が存在せず、variant 拡張が不自然になる。独立型で意図を明示する。
- **破壊的変更**: `ActorContext::watch` / `watch_with` の戻り値が `Result<(), SendError>` → `Result<(), WatchRegistrationError>` に変わる。CLAUDE.md の「後方互換は不要」方針に従い受容する。
- **代替**: `SendError::DuplicateWatch { target: Pid, user_message: Option<AnyMessage> }` variant 拡張。却下理由は上記 (意味の混線)。

### Decision 2: `watching` + `watch_with_messages` の論理合成関数で「previous watch state」を判定

**実装の前提 (コード調査結果)**:
- `ActorCellState::watching` は `Vec<(Pid, WatchKind)>` であり、同一 pid を `(P, User)` + `(P, Supervision)` の 2 entry として保持しうる (`actor_cell_state.rs:30,90-93`)。
- 既存 `fn watching_contains_pid(&self, pid: Pid) -> bool` (`actor_cell_state.rs:85-87`) は **any kind** を判定するため、User 限定判定に流用不可。
- したがって **User 限定の query helper を新設**する必要がある。

**追加する API**:
- `ActorCellState::watching_contains_user(&self, pid: Pid) -> bool` — `watching` を走査し、`(existing_pid, WatchKind::User)` 一致のみ true を返す。
- `modules/actor-core/src/core/kernel/actor/watch_registration_kind.rs` に以下を新設:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub(crate) enum WatchRegistrationKind {
    None,
    Plain,
    WithMessage,
  }
  ```
- `ActorCell::watch_registration_kind(&self, target: Pid) -> WatchRegistrationKind`:
  ```rust
  self.state.with_read(|state| {
    if !state.watching_contains_user(target) {
      WatchRegistrationKind::None
    } else if state.watch_with_messages.iter().any(|(p, _)| *p == target) {
      WatchRegistrationKind::WithMessage
    } else {
      WatchRegistrationKind::Plain
    }
  })
  ```
- **Rationale**: 既存の split data structure を壊さずに Pekko 風 `Option[Any]` 意味論を合成する。`with_read` を使い write lock を取らないので衝突 cost は低い。
- **Supervision 用 watch (kernel 内部) は除外**: `WatchKind::Supervision` は親 → 子の supervision path で使われる internal watch。`watching_contains_user` により User 限定で判定することで、supervision entry が duplicate 判定に混入しない。
- **配置**: `WatchRegistrationKind` は `pub(crate)` のため公開 API には晒さない。`actor/watch_kind.rs` 隣接の `actor/watch_registration_kind.rs` に置く (既存 `WatchKind` 命名と整合)。

### Decision 3: `watch` / `watch_with` 内のチェック順序と rollback

- **`watch(target)` 新フロー**:
  ```
  1. if target == self: Ok(())
  2. match cell.watch_registration_kind(target.pid()):
     | None         => proceed (register + send Watch)
     | Plain        => Ok(())  // Pekko parity: None == None, idempotent
     | WithMessage  => Err(Duplicate { conflict: WatchWithThenPlain })
  ```
- **`watch_with(target, msg)` 新フロー**:
  ```
  1. if target == self: Ok(())
  2. match cell.watch_registration_kind(target.pid()):
     | None         => proceed (register_watch_with + register_watching + send Watch)
     | Plain        => Err(Duplicate { conflict: PlainThenWatchWith })  // without taking ownership of msg
     | WithMessage  => Err(Duplicate { conflict: WatchWithThenWatchWith })
  3. on watch send failure: rollback watch_with_messages entry (既存挙動維持)
  ```
- **Rationale**: register 前に duplicate check を行うことで、silent overwrite が構造的に発生しない。err 時に msg は `WatchRegistrationError::Duplicate` の payload に載せず破棄する (SendError と違い復元不要; caller は既に msg を保持していないケースもある)。

### Decision 4: `register_watch_with` の silent overwrite 廃止

- **選択**: `register_watch_with` は引数を assert_unique で受ける。既に entry があれば **panic する**。
- **Rationale**: 上位 `watch_with` が `watch_registration_kind` で事前チェックするため、下位 register 時に衝突があること自体が不変条件違反。pub(crate) API なので panic で十分。
- **代替**: Err を返す設計も考えたが、Decision 3 の事前チェックで到達不能なため panic で不変条件を表明する方が読みやすい。

### Decision 5: conservative strategy の明示ドキュメント

- `watch_with` → `watch_with` は Pekko が「同一 msg なら `Some(m1) == Some(m2)` で許容」するが、fraktor-rs は `AnyMessage` 同値判定不能のため常に `WatchWithThenWatchWith` エラーにする。
- この divergence は rustdoc と design.md の本 Decision で明記する。ユーザーに対しては「同じ relate を再設定したい場合も必ず `unwatch` を先に呼ぶ」ことを case-by-case で説明する。
- 将来 `AnyMessage` に `Message + PartialEq` bound の typed 版が整備されたら、strict equality 経路を追加して divergence を閉塞できる。本 change では futureproof だけ意識する。

### Decision 6: `WatchRegistrationError` から `ActorError` への変換経路

**問題**: 既存 caller のうち guardian 系 (`root_guardian_actor.rs:31`, `system_guardian_actor.rs:79,125`) は `map_err(|error| ActorError::from_send_error(&error))` パターンで `SendError` を `ActorError` に変換している。戻り値を `WatchRegistrationError` に変えると、`Send(SendError)` variant 経路は既存方式で変換できるが、`Duplicate { .. }` variant の `ActorError` 化指針が未定義。

**選択**: `WatchRegistrationError::to_actor_error(&self) -> ActorError` ヘルパーを実装し、variant ごとに変換先を定義する:
- `Send(se)` → `ActorError::from_send_error(se)` (既存パス流用)
- `Duplicate { target, conflict }` → `ActorError::recoverable(format!("duplicate watch registration on {target:?}: {conflict:?}"))`

**Rationale**:
- duplicate 衝突はプログラミングミス (unwatch 忘れ) なので、actor panic 相当の `ActorError::fatal` でもよいが、guardian のような長命 actor で panic すると system 全体が死ぬ。`recoverable` として supervisor が判断できる形に留める。
- caller 側は `map_err(|e| e.to_actor_error())` で 1 行の書換えで追従可能。既存の `from_send_error` ラッパー置換として完結する。

**代替**:
- `From<WatchRegistrationError> for ActorError` impl (自動変換)。却下理由: `ActorError::from_send_error(&error)` のように参照ベースの既存 idiom と不整合。
- `Duplicate` を panic 化。却下理由: guardian 系で panic すると system が落ちるため、recoverable error で留める方が堅牢。

**caller 追従パターン**:
```rust
// 既存
ctx.watch(&actor).map_err(|error| ActorError::from_send_error(&error))?;
// 新 (Decision 6 適用後)
ctx.watch(&actor).map_err(|error| error.to_actor_error())?;
```

## Risks / Trade-offs

### Risk 1: 既存 caller の破壊的変更

- **影響**: `ActorContext::watch` / `watch_with` の戻り値が変わる。typed/untyped 両方の caller が追随修正が必要。
- **緩和**: 既存 callers (`actor_context.rs` 以外で `.watch(` / `.watch_with(` を呼ぶコード) を grep で網羅し、全箇所 `?` or explicit match を正しく書き換える。CI の `./scripts/ci-check.sh ai all` で完全性を担保する。

### Risk 2: Supervision watch との混同

- **影響**: `watch_registration_kind` は User watch のみを返すが、実装者が Supervision 用 register_watching と混同すると False Negative (duplicate 検出漏れ) が起きる。
- **緩和**: `watch_registration_kind` の rustdoc で「User watch only」を太字で明記し、実装コメントにも対応 `WatchKind::User` 値を示す。テストで supervision 対象を duplicate-check 対象外として検証する。

### Risk 3: conservative strategy による false-negative (Pekko が許容するケースの拒否)

- **影響**: `watchWith(a, same_m)` × 2 回呼ぶコードは Pekko では no-op だが、fraktor-rs では `WatchWithThenWatchWith` エラーになる。典型的コーディングスタイルからは外れるが、idempotent 再設定パターンを使う user は修正が必要。
- **緩和**: divergence を design Decision 5 で明示。rustdoc にも記載。
- **受容**: `AnyMessage` 同値判定を後付けで導入するコストが本 change の scope を大きく超えるため、本 change では受容。

### Risk 4: 重複検出の race / TOCTOU

- **影響**: `watch_registration_kind` と `register_watching` の間で別 thread が watching を書き換えると、事前チェックが通った後に duplicate 状態になる可能性。
- **緩和**: 実質的に発生しない。`actor_context.rs::watch` / `watch_with` は自 actor の context 上でのみ呼ばれ、`watching` の書き換えも同一 cell の `state.with_write` を介すため、`&mut self` プラス cell 内 lock で serialized。read → check → write の window は内部閉鎖済。
- **補強策** (optional): `ActorCell` に `check_and_register_watch(target, kind)` のような compound atomic 関数を追加して、チェック + 登録を 1 回の `with_write` で行う。設計簡潔さと parity を優先し、本 change では compound は導入せず、2 ステップで十分な保証を rustdoc に明記する。後続 refactor で必要性が生じたら追加する。

### Risk 5: address-terminated 購読の欠落を gap-analysis で見失う

- **影響**: AC-M4 の 2 責務のうち片方だけ close した場合、将来「AC-M4 完了」と誤認して残存 gap を見落とす懸念。
- **緩和**: gap-analysis を `AC-M4a` / `AC-M4b` に分割する (本 change tasks.md Phase 9)。`AC-M4b` には `n/a until remote/cluster` と明記し、remote capability 完成時の reopen 条件を記述する。

### Risk 6: restart 時の `drop_watch_with_messages` 非対称により post_restart での再 watch_with が Err(PlainThenWatchWith) になる

- **問題**: `actor_cell.rs:1035` の `drop_watch_with_messages` は `watch_with_messages.clear()` のみ行い、`state.watching` の User entry は **クリアしない**。restart 後も watching には `(pid, User)` が残るが watch_with_messages は空。この状態で `post_restart` が `ctx.watch_with(prev_target, new_msg)` を呼ぶと、`watch_registration_kind` は `Plain` を返し、本 change の新仕様で `WatchConflict::PlainThenWatchWith` エラーを返す。
- **Pekko との差分**: Pekko は `FaultHandling.scala:200-213` の `unwatchWatchedActors` を `preRestart` / fault 時に呼び、`watching = Map.empty` で completely reset する。fraktor-rs は watching を restart で clear しないため、この divergence は **本 change の導入より前から存在する pre-existing な挙動差**。
- **本 change の扱い**: scope 外とする。理由: (a) `watching` clear は AL-H1 / AC-H3 で既に決定済の restart semantics に依存し、本 change で触ると範囲が膨らむ。(b) 現時点の production user で「post_restart で同一 target に watch_with 再登録」するパターンは確認されていない。(c) 発生しても `Err` を観測でき silent でない。
- **ユーザー影響緩和**: rustdoc に「restart を跨いだ watch 再登録が必要な場合は `post_restart` 内で先に `unwatch` を呼ぶこと」を明示する (Phase 7.1 で追記)。
- **将来対応**: 別 change `pekko-death-watch-restart-symmetry` で `drop_watch_with_messages` を `clear_all_user_watches` に改名し、`watching[User]` と `watch_with_messages` を同時に clear する設計。remote の AC-M4b と合わせて fault-path 一連の整備として検討。gap-analysis に記録。
