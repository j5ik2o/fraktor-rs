## Context

### fraktor-rs の現状

`modules/persistence-core` は no_std core として classic persistence 相当の API を提供している。

- `Eventsourced`
- `PersistentActor`
- `PersistentActorAdapter`
- `PersistenceContext`
- `Journal` / `SnapshotStore`
- `JournalActor` / `SnapshotActor`
- `InMemoryJournal` / `InMemorySnapshotStore`

一方、`docs/gap-analysis/persistence-gap-analysis.md` では typed write-side API が未実装として整理されている。Pekko の現行推奨 API は `EventSourcedBehavior` だが、fraktor-rs へそのまま移植すると、typed actor の通常 DSL と persistence DSL が分離し、DDD 集約を自然に扱いにくい。

### 参照する設計

`pekko-persistence-effector` は以下の設計を採る。

- 集約 actor は通常の typed `Behavior` として実装する
- 永続化用 actor は集約 actor の child として隠蔽する
- recovery では永続化用 actor が state を復元し、復元後の state と effector を集約 actor に渡す
- command handler はドメインオブジェクトを呼び出して新 state と event を得る
- event persist が成功したあと、command handler が返信し、次の `Behavior` に新 state を渡す

fraktor-rs でもこの方向を採用する。ただし Rust では Scala の `PartialFunction` / path-dependent DSL を再現せず、型付き builder と closure で表現する。

## Goals / Non-Goals

**Goals:**

- typed actor が通常の `Behavior<M>` として書ける persistence API を提供する
- domain command handling と event persistence を分離する
- command handler 内でドメインオブジェクトが返した新 state を直接利用できる
- recovery / persist / snapshot 中の stashing 契約を明確にする
- `Persisted` / `Ephemeral` / `Deferred` を同一 API で切り替えられる
- no_std core 境界を守る

**Non-Goals:**

- `EventSourcedBehavior` の完全互換実装
- `Effect` chain DSL の導入
- durable state typed behavior
- read-side query
- std filesystem snapshot store

## Decisions

### Decision 1: typed persistence の推奨 API は `PersistenceEffector` にする

**選択:** `modules/persistence-core/src/core/typed/` に `PersistenceEffector<S, E, M>` を追加する。

想定 public API:

```rust
pub struct PersistenceEffector<S, E, M> { /* private fields */ }

impl<S, E, M> PersistenceEffector<S, E, M>
where
  S: Send + Sync + 'static,
  E: Send + Sync + Clone + 'static,
  M: Send + Sync + 'static,
{
  pub fn persist_event<F>(
    &self,
    ctx: &mut TypedActorContext<'_, M>,
    event: E,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: Fn(&E) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static;

  pub fn persist_events<F>(
    &self,
    ctx: &mut TypedActorContext<'_, M>,
    events: Vec<E>,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: Fn(&[E]) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static;

  pub fn persist_snapshot<F>(
    &self,
    ctx: &mut TypedActorContext<'_, M>,
    snapshot: S,
    force: bool,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: Fn(&S) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static;
}
```

**Rationale:**

- Rust の `Behavior` は message handler を `Fn` として保持するため、callback は `FnOnce` ではなく `Fn` を基本契約にする
- `E: Clone` は persist 成功通知、snapshot criteria、in-memory replay で event を再利用するため要求する
- aggregate actor は `Behavior<M>` を返すだけでよく、`EventSourcedBehavior` 専用 DSL を学習しなくてよい

### Decision 2: `PersistenceEffector::from_config` は recovery 完了後に `(state, effector)` を渡す

**選択:** entrypoint は `from_config` とし、`on_ready` closure が初期 behavior を返す。

```rust
impl<S, E, M> PersistenceEffector<S, E, M> {
  pub fn from_config<F>(config: PersistenceEffectorConfig<S, E, M>, on_ready: F) -> Behavior<M>
  where
    F: Fn(S, PersistenceEffector<S, E, M>) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static;
}
```

**Rationale:**

- recovery は actor 起動直後に必ず完了してから command processing へ移る
- recovery 中に届いた user command は stash し、`on_ready` の behavior へ unstash する
- `on_ready` は state pattern matching / state-specific handler 分岐の唯一の入口になる

### Decision 3: 永続化用 child actor は classic persistence 基盤を使う

**選択:** `Persisted` mode では typed aggregate actor の child として `PersistenceStoreActor<S, E>` 相当を起動し、その内部で既存 `PersistentActor` / `PersistenceContext` / journal / snapshot actor を使う。

内部 protocol:

```rust
enum PersistenceStoreCommand<S, E> {
  PersistEvent { event: E, reply_to: TypedActorRef<PersistenceStoreReply<S, E>> },
  PersistEvents { events: Vec<E>, reply_to: TypedActorRef<PersistenceStoreReply<S, E>> },
  PersistSnapshot { snapshot: S, reply_to: TypedActorRef<PersistenceStoreReply<S, E>> },
  DeleteSnapshots { to_sequence_nr: u64, reply_to: TypedActorRef<PersistenceStoreReply<S, E>> },
}

enum PersistenceStoreReply<S, E> {
  RecoveryCompleted { state: S, sequence_nr: u64 },
  PersistedEvents { events: Vec<E>, sequence_nr: u64 },
  PersistedSnapshot { snapshot: S, sequence_nr: u64 },
  DeletedSnapshots { to_sequence_nr: u64 },
  Failed { error: PersistenceError },
}
```

**Rationale:**

- classic persistence はすでに `PersistentActorAdapter` と `PersistenceContext` に recovery / write / stash 制御を持つ
- typed API は永続化 store を新しく発明せず、既存 runtime を隠蔽する thin layer にする
- recovery 時の `apply_event(&S, &E) -> S` は store actor 側でのみ実行し、aggregate actor の command handler は再実行しない

### Decision 4: aggregate actor と store reply は message converter で接続する

**選択:** `PersistenceEffectorMessageConverter<S, E, M>` を config に持たせる。

```rust
pub struct PersistenceEffectorMessageConverter<S, E, M> {
  pub wrap_store_reply: ArcShared<dyn Fn(PersistenceStoreReply<S, E>) -> M + Send + Sync>,
  pub unwrap_store_reply: ArcShared<dyn Fn(&M) -> Option<PersistenceStoreReply<S, E>> + Send + Sync>,
}
```

**Rationale:**

- fraktor-rs の typed `message_adapter` を使う場合でも、最終的に aggregate actor の message 型 `M` に store reply を包む必要がある
- user command と internal persistence reply を同じ `Behavior<M>` が扱える
- Scala 版の `MessageConverter` と同等の責務を Rust の closure で表す

### Decision 5: persist 中は waiting behavior が user command を stash する

**選択:** `persist_event` / `persist_events` / `persist_snapshot` は store actor へ command を送り、internal reply を待つ `Behavior<M>` を返す。この waiting behavior は次を満たす。

- `unwrap_store_reply(message)` が対象 reply を返した場合だけ callback を実行する
- その他の user command は `ctx.stash_with_limit(config.stash_capacity)` で stash する
- callback が返した behavior へ `ctx.unstash_all()` する

**Rationale:**

- classic `persist` と同じ fencing semantics を typed actor で再現する
- persist 完了前に次 command が現在 state を見て処理される事故を防ぐ
- `persist_unfenced` 相当は本 change では追加しない

### Decision 6: `Ephemeral` は同じ semantics を process 内 store で再現する

**選択:** `Ephemeral` mode は journal plugin を使わず、effector 内部の in-memory event / snapshot store へ書き込む。recovery は同じ `apply_event` と snapshot criteria を通る。

**Rationale:**

- 開発初期に persistence extension / plugin を設定せずに aggregate logic をテストできる
- `Persisted` と同じ callback / stashing / snapshot criteria を維持する
- `InMemoryJournal` を使う integration test とは別に、effector 固有の lightweight mode として扱う

### Decision 7: `Deferred` は no-op persist として扱う

**選択:** `Deferred` mode は event / snapshot を保存せず、callback を即時実行する。

**Rationale:**

- upstream `pekko-persistence-effector` と同じ dry-run mode を提供する
- performance test や persistence temporarily disabled の用途を満たす
- `Deferred` では recovery は常に `initial_state`

### Decision 8: snapshot / retention は effector の責務に含める

**選択:** `SnapshotCriteria<S, E>` と `RetentionCriteria` を typed effector 層に追加する。

```rust
pub enum SnapshotCriteria<S, E> {
  Never,
  Always,
  Every { number_of_events: u64 },
  Predicate(ArcShared<dyn Fn(Option<&E>, &S, u64) -> bool + Send + Sync>),
}

pub struct RetentionCriteria {
  pub snapshot_every: Option<u64>,
  pub keep_snapshots: Option<u64>,
}
```

**Rationale:**

- event persist と snapshot 判定は同じ sequence number を共有する
- snapshot 判定は command handler に散らさず effector に閉じる
- retention は snapshot 成功後の cleanup として扱う

### Decision 9: persistence failure と domain error を混ぜない

**選択:** domain validation error は user command handler が reply して `Behaviors::same()` 等を返す。persistence failure は `PersistenceStoreReply::Failed` から `ActorError::fatal` に変換し、actor を停止させるのを default とする。

**Rationale:**

- persistence failure は「event が保存されていない」ため、domain success と同じ reply を返してはならない
- failure retry / backoff は child store actor の supervision / `BackoffConfig` で扱う
- user-visible domain error と infrastructure error の境界を保つ

### Decision 10: module / file layout は 1file1type を守る

**選択:** 新規型は原則として独立ファイルに置く。

想定配置:

```text
modules/persistence-core/src/core/typed.rs
modules/persistence-core/src/core/typed/persistence_id.rs
modules/persistence-core/src/core/typed/persistence_mode.rs
modules/persistence-core/src/core/typed/persistence_effector.rs
modules/persistence-core/src/core/typed/persistence_effector_config.rs
modules/persistence-core/src/core/typed/persistence_effector_message_converter.rs
modules/persistence-core/src/core/typed/snapshot_criteria.rs
modules/persistence-core/src/core/typed/retention_criteria.rs
modules/persistence-core/src/core/typed/backoff_config.rs
modules/persistence-core/src/core/typed/internal/persistence_store_actor.rs
modules/persistence-core/src/core/typed/internal/persistence_store_command.rs
modules/persistence-core/src/core/typed/internal/persistence_store_reply.rs
```

**Rationale:**

- repo の dylint / 1file1type 方針に合わせる
- public API と internal protocol の境界を明確にする

## State Machine

effector wrapper の状態は以下を持つ。

```text
Starting
  -> Recovering
  -> Ready(state, sequence_nr)
  -> Persisting(kind, pending_callback)
  -> Ready(new_state, sequence_nr)
  -> Failed(error)
```

- `Starting`: typed behavior 起動直後
- `Recovering`: store actor の recovery 完了待ち。user command は stash
- `Ready`: aggregate behavior が command を処理可能
- `Persisting`: persist / snapshot / delete の完了待ち。user command は stash
- `Failed`: fatal error として actor 停止

## Risks / Trade-offs

### Risk 1: `Fn` callback 制約が使いにくい可能性

typed `Behavior` の handler が `Fn` を要求するため、callback も `Fn` に寄せる。`FnOnce` でないため、reply target や new state は clone 可能な handle / value として捕捉する必要がある。必要なら実装時に internal `OnceCallback` wrapper を検討するが、初期仕様では `Fn` を採用する。

### Risk 2: `PersistenceEffector` 名

命名規約では曖昧サフィックスを避けるが、`PersistenceEffector` は upstream ライブラリ名そのものなので採用する。rustdoc では「永続化副作用を起動する handle」と責務を明記する。

### Risk 3: typed `EventSourcedBehavior` parity から離れる

Pekko typed API の移植率としては `EventSourcedBehavior` 未実装のまま残る。ただし本 change は fraktor-rs の推奨 typed persistence として effector pattern を定義する。Pekko API 互換 wrapper は別 capability として扱う。

### Risk 4: in-memory mode と existing `InMemoryJournal` の重複

`Ephemeral` は effector-level development mode、`InMemoryJournal` は persistence plugin としての journal 実装であり責務が異なる。仕様上は両方を残す。
