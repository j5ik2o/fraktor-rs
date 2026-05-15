## Context

### fraktor-rs の現状

現行 `modules/persistence-core` は no_std core として classic persistence 相当の API を提供している。本 change ではこの crate を `modules/persistence-core-kernel` / `fraktor-persistence-core-kernel-rs` へ rename し、actor core と同じ kernel 命名へ揃える。

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
- command handler 内でドメインオブジェクトが返した新 state を clone せず one-shot callback に move して直接利用できる
- recovery / persist / snapshot 中の stashing 契約を明確にする
- `Persisted` / `Ephemeral` / `Deferred` を同一 API で切り替えられる
- no_std core 境界を守る
- closed Rust enum の制約に合わせ、内部 store protocol と user message `M` の結合点を安定した signal adapter に限定する

**Non-Goals:**

- `EventSourcedBehavior` の完全互換実装
- `Effect` chain DSL の導入
- durable state typed behavior
- read-side query
- std filesystem snapshot store

## Decisions

### Decision 1: persistence crate は kernel / typed に分離する

**選択:** 現行 `modules/persistence-core/` を `modules/persistence-core-kernel/` へ rename し、Pekko classic persistence 相当の kernel crate とする。typed write-side API は新 crate `modules/persistence-core-typed/` に追加する。

actor runtime の依存方向は以下に固定する。

```text
fraktor-persistence-core-typed-rs
  -> fraktor-persistence-core-kernel-rs
  -> fraktor-actor-core-kernel-rs

fraktor-persistence-core-typed-rs
  -> fraktor-actor-core-typed-rs
```

`fraktor-persistence-core-kernel-rs` は `fraktor-actor-core-typed-rs` に依存してはならない。`fraktor-utils-core-rs` や `ahash` / `hashbrown` のような no_std 対応の補助 crate はこの actor runtime 境界とは別扱いで、既存の core 実装に必要なら利用してよい。

### Decision 2: typed persistence の推奨 API は `PersistenceEffector` にする

**選択:** `modules/persistence-core-typed/src/` に `PersistenceEffector<S, E, M>` を追加する。`Behavior<M>` と `TypedActorContext<'_, M>` は typed crate 側でのみ `fraktor_actor_core_typed_rs` から参照する。

想定 public API:

```rust
pub struct PersistenceEffector<S, E, M> { /* private fields */ }

impl<S, E, M> PersistenceEffector<S, E, M>
where
  S: Send + Sync + Clone + 'static,
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
    F: FnOnce(&E) -> Result<Behavior<M>, ActorError> + Send + 'static;

  pub fn persist_events<F>(
    &self,
    ctx: &mut TypedActorContext<'_, M>,
    events: Vec<E>,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: FnOnce(&[E]) -> Result<Behavior<M>, ActorError> + Send + 'static;

  pub fn persist_snapshot<F>(
    &self,
    ctx: &mut TypedActorContext<'_, M>,
    snapshot: S,
    force: bool,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: FnOnce(&S) -> Result<Behavior<M>, ActorError> + Send + 'static;

  pub fn persist_event_with_snapshot<F>(
    &self,
    ctx: &mut TypedActorContext<'_, M>,
    event: E,
    snapshot: S,
    force_snapshot: bool,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: FnOnce(&E) -> Result<Behavior<M>, ActorError> + Send + 'static;

  pub fn persist_events_with_snapshot<F>(
    &self,
    ctx: &mut TypedActorContext<'_, M>,
    events: Vec<E>,
    snapshot: S,
    force_snapshot: bool,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: FnOnce(&[E]) -> Result<Behavior<M>, ActorError> + Send + 'static;
}
```

**Rationale:**

- user callback は persist operation ごとに一度だけ実行されるため `FnOnce` を基本契約にする。これにより command handler が作った `new_state` を clone せず callback へ move でき、`pekko-persistence-effector` の aggregate actor style に近づく
- fraktor-rs の `Behavior` handler 自体は `Fn` だが、waiting behavior は private `ExtensibleBehavior` / `PendingContinuation` として実装し、内部で `Option<Box<dyn FnOnce...>>` を消費する
- `S: Clone` / `E: Clone` は typed behavior が message を `&M` として扱うこと、recovery signal から `on_ready` へ owned state を渡すこと、in-memory replay / snapshot criteria で値を再利用することのため要求する。command-derived `new_state` の clone は user callback へ要求しない
- aggregate actor は `Behavior<M>` を返すだけでよく、`EventSourcedBehavior` 専用 DSL を学習しなくてよい

### Decision 3: `PersistenceEffector::from_config` は recovery 完了後に `(state, effector)` を渡す

**選択:** entrypoint は `from_config` とし、`on_ready` closure が初期 behavior を返す。

```rust
impl<S, E, M> PersistenceEffector<S, E, M> {
  pub fn props<F>(config: PersistenceEffectorConfig<S, E, M>, on_ready: F) -> TypedProps<M>
  where
    F: Fn(S, PersistenceEffector<S, E, M>) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static;

  pub fn from_config<F>(config: PersistenceEffectorConfig<S, E, M>, on_ready: F) -> Behavior<M>
  where
    F: Fn(S, PersistenceEffector<S, E, M>) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static;
}
```

**Rationale:**

- recovery は actor 起動直後に必ず完了してから command processing へ移る
- recovery 中に届いた user command は stash し、`on_ready` の behavior へ unstash する
- `on_ready` は state pattern matching / state-specific handler 分岐の唯一の入口になる
- `props` は config / `on_ready` を内部の共有 handle として保持し、`TypedProps::from_behavior_factory(...).with_stash_mailbox()` 相当を返す。通常の spawn 経路では stash mailbox 契約を API 側で満たす
- `from_config` は advanced / composition 用の低レベル behavior builder として残す。これを直接使う caller は同等の stash mailbox props を適用する必要がある

### Decision 4: 永続化用 child actor は classic persistence 基盤を使う

**選択:** `Persisted` mode では typed aggregate actor の child として `PersistenceStoreActor<S, E>` 相当を起動し、その内部で `fraktor-persistence-core-kernel-rs` の `PersistentActor` / `PersistenceContext` / journal / snapshot actor を使う。

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

### Decision 5: aggregate actor と effector signal は message adapter で接続する

**選択:** internal store protocol は public API へ出さず、stable な `PersistenceEffectorSignal<S, E>` と `PersistenceEffectorMessageAdapter<S, E, M>` を config に持たせる。

```rust
pub enum PersistenceEffectorSignal<S, E> {
  RecoveryCompleted { state: S, sequence_nr: u64 },
  PersistedEvents { events: Vec<E>, sequence_nr: u64 },
  PersistedSnapshot { snapshot: S, sequence_nr: u64 },
  DeletedSnapshots { to_sequence_nr: u64 },
  Failed { error: PersistenceError },
}

pub struct PersistenceEffectorMessageAdapter<S, E, M> {
  pub wrap_signal: ArcShared<dyn Fn(PersistenceEffectorSignal<S, E>) -> M + Send + Sync>,
  pub unwrap_signal: ArcShared<dyn for<'a> Fn(&'a M) -> Option<&'a PersistenceEffectorSignal<S, E>> + Send + Sync>,
}
```

**Rationale:**

- Rust の enum は Scala 版の unchecked default converter のように後から variant を足せないため、actor-private message 型 `M` は effector signal を包める必要がある
- ただし user-facing API に `PersistenceStoreReply` を露出させると hidden child の実装詳細が漏れるため、public 境界は `PersistenceEffectorSignal` に限定する
- showcase では domain command と actor-private persistence signal を同じ enum に入れる場合でも、domain object / domain command API とは分離する
- `unwrap_signal` は `&M` から borrowed signal を返す。owned state / event が必要な箇所では `S: Clone` / `E: Clone` を使い、command handler が作った `new_state` の clone は要求しない

### Decision 6: persist 中は waiting behavior が user command を stash する

**選択:** `persist_event` / `persist_events` / `persist_snapshot` は store actor へ command を送り、internal reply を待つ `Behavior<M>` を返す。この waiting behavior は次を満たす。

- `unwrap_signal(message)` が対象 signal を返した場合だけ callback を実行する
- その他の user command は `StashBuffer<M>::stash(ctx)` で stash する
- callback が返した behavior へ `StashBuffer<M>::unstash_all(ctx)` する
- `from_config` は内部で `Behaviors::with_stash(config.stash_capacity, ...)` 相当の wrapper を構築する
- spawn 時は `PersistenceEffector::props(config, on_ready)` を使うことを推奨し、この helper が `TypedProps::with_stash_mailbox()` を必ず適用する。`from_config` を直接使う advanced caller だけが stash mailbox props を明示する

**Rationale:**

- classic `persist` と同じ fencing semantics を typed actor で再現する
- persist 完了前に次 command が現在 state を見て処理される事故を防ぐ
- `persist_unfenced` 相当は本 change では追加しない

### Decision 7: `Ephemeral` は同じ semantics を actor-system extension store で再現する

**選択:** `Ephemeral` mode は journal plugin を使わず、actor-system extension が所有する in-memory event / snapshot store へ書き込む。store は persistence id で partition し、同じ actor system 内で同じ persistence id の actor を再作成した場合だけ replay できる。recovery は同じ `apply_event` と snapshot criteria を通る。

実装は `PersistenceEffector` の setup 時に `TypedActorContext::system()` から internal `EphemeralPersistenceStoreExtension` を解決する。extension は type-erased な event / snapshot 値を `ArcShared` + 既存 sync primitive で保持し、process global singleton にはしない。テストは actor system を作り直すことで store scope を分離できる。

**Rationale:**

- 開発初期に persistence extension / plugin を設定せずに aggregate logic をテストできる
- `Persisted` と同じ callback / stashing / snapshot criteria を維持する
- `InMemoryJournal` を使う integration test とは別に、effector 固有の lightweight mode として扱う
- process global singleton にしないことで no_std core 境界と test isolation を守る

### Decision 8: `Deferred` は no-op persist として扱う

**選択:** `Deferred` mode は event / snapshot を保存せず、callback を即時実行する。

**Rationale:**

- upstream `pekko-persistence-effector` と同じ dry-run mode を提供する
- performance test や persistence temporarily disabled の用途を満たす
- `Deferred` では recovery は常に `initial_state`

### Decision 9: snapshot / retention は effector の責務に含める

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

### Decision 10: persistence failure と domain error を混ぜない

**選択:** domain validation error は user command handler が reply して `Behaviors::same()` 等を返す。persistence failure は internal store reply から `PersistenceEffectorSignal::Failed` へ変換し、最終的に `ActorError::fatal` として actor を停止させるのを default とする。

**Rationale:**

- persistence failure は「event が保存されていない」ため、domain success と同じ reply を返してはならない
- failure retry / backoff は child store actor の supervision / `BackoffConfig` で扱う
- user-visible domain error と infrastructure error の境界を保つ

### Decision 11: module / file layout は 1file1type を守る

**選択:** 新規型は原則として独立ファイルに置く。

想定配置:

```text
modules/persistence-core-kernel/src/**              # existing classic persistence code, moved from persistence-core
modules/persistence-core-typed/src/lib.rs
modules/persistence-core-typed/src/persistence_id.rs
modules/persistence-core-typed/src/persistence_mode.rs
modules/persistence-core-typed/src/persistence_effector.rs
modules/persistence-core-typed/src/persistence_effector_config.rs
modules/persistence-core-typed/src/persistence_effector_signal.rs
modules/persistence-core-typed/src/persistence_effector_message_adapter.rs
modules/persistence-core-typed/src/snapshot_criteria.rs
modules/persistence-core-typed/src/retention_criteria.rs
modules/persistence-core-typed/src/backoff_config.rs
modules/persistence-core-typed/src/internal/persistence_store_actor.rs
modules/persistence-core-typed/src/internal/persistence_store_command.rs
modules/persistence-core-typed/src/internal/persistence_store_reply.rs
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
  -> Persisting(kind, pending_once_callback)
  -> Ready(new_state, sequence_nr)
  -> Failed(error)
```

- `Starting`: typed behavior 起動直後
- `Recovering`: store actor の recovery 完了待ち。user command は stash
- `Ready`: aggregate behavior が command を処理可能
- `Persisting`: persist / snapshot / delete の完了待ち。user command は stash
- `Failed`: fatal error として actor 停止

## Risks / Trade-offs

### Risk 1: one-shot callback を `Behavior` の `Fn` handler と接続する必要がある

user callback は `FnOnce` にするが、typed `Behavior` の handler は `Fn` を要求する。実装では private `ExtensibleBehavior` と `PendingContinuation` を使い、pending callback を `Option` から一度だけ取り出す。これにより callback 実行前の重複 signal は fatal error として検出できる。

### Risk 2: `PersistenceEffector` 名

命名規約では曖昧サフィックスを避けるが、`PersistenceEffector` は upstream ライブラリ名そのものなので採用する。rustdoc では「永続化副作用を起動する handle」と責務を明記する。

### Risk 3: typed `EventSourcedBehavior` parity から離れる

Pekko typed API の移植率としては `EventSourcedBehavior` 未実装のまま残る。ただし本 change は fraktor-rs の推奨 typed persistence として effector pattern を定義する。Pekko API 互換 wrapper は別 capability として扱う。

### Risk 4: in-memory mode と existing `InMemoryJournal` の重複

`Ephemeral` は effector-level development mode、`InMemoryJournal` は persistence plugin としての journal 実装であり責務が異なる。仕様上は両方を残す。

### Risk 5: Rust では Scala 版の unchecked default message converter を再現できない

Scala 版は default converter が marker trait 付き内部 message を `M` に cast できるが、Rust の closed enum では同じ手法を採らない。代わりに actor-private message 型へ `PersistenceEffectorSignal` を包む adapter を明示する。これは少し boilerplate を増やすが、store actor の内部 protocol は公開しない。
