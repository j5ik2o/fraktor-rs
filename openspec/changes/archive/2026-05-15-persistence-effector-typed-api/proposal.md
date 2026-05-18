## Why

現行 `modules/persistence-core` は classic 相当の `PersistentActor` / journal / snapshot / recovery を持つが、typed write-side API はまだ存在しない。Pekko の `EventSourcedBehavior` / `Effect` 体系をそのまま持ち込むと、`pekko-persistence-effector` が解いた以下の課題を fraktor-rs でも再発させる。

- 通常の `Behavior` ベースの typed actor と異なる専用 DSL を強制する
- 状態が増えるほど command handler / event handler が肥大化しやすい
- ドメインオブジェクトが返した新状態を command handler 側で自然に使いにくく、ドメインロジックを event handler 側にも重複させがちになる

fraktor-rs はまだ正式リリース前であり、Pekko parity を機械的に追うより、Rust の typed actor と DDD 集約を自然につなぐ API を先に定義する方がよい。そこで、typed 永続化 API は `EventSourcedBehavior` 直輸入ではなく、`pekko-persistence-effector` の「集約 actor は通常の typed `Behavior`、永続化は子 actor / effector が担当する」仕様を採用する。

## What Changes

### 1. persistence core を kernel / typed に分離する

現行 `modules/persistence-core/` を `modules/persistence-core-kernel/` へ改名し、Pekko classic persistence 相当の core として typed actor 層から独立させる。`fraktor-persistence-core-kernel-rs` は actor runtime 境界として `fraktor-actor-core-kernel-rs` を使い、`fraktor-actor-core-typed-rs` に依存してはならない。`fraktor-utils-core-rs` や no_std 対応の補助 crate への依存はこの制約に含めない。

`modules/persistence-core-typed/` を新設し、typed write-side API を追加する。新 crate `fraktor-persistence-core-typed-rs` は no_std のまま `fraktor-persistence-core-kernel-rs` と `fraktor-actor-core-typed-rs` を合成し、typed actor facade と persistence kernel を接続する。

- `PersistenceId`
- `PersistenceMode`
- `PersistenceEffectorConfig<S, E, M>`
- `PersistenceEffector<S, E, M>`
- `PersistenceEffectorSignal<S, E>`
- `SnapshotCriteria<S, E>`
- `RetentionCriteria`
- `BackoffConfig`
- `PersistenceEffectorMessageAdapter<S, E, M>`

この crate は `fraktor_actor_core_typed_rs::{Behavior, dsl::Behaviors}` と `fraktor_actor_core_typed_rs::actor::TypedActorContext` と連携し、ユーザーの actor は `Behaviors::setup` / `Behaviors::receive_message_partial` / 状態別 handler 分割をそのまま使える。

### 2. 永続化 store actor を hidden child として扱う

`PersistenceMode::Persisted` では、effector が内部で永続化専用 child actor を起動する。child actor は `persistence-core-kernel` の classic persistence 基盤 (`PersistentActor`, `PersistenceContext`, journal / snapshot actor) を使い、typed aggregate actor には以下だけを返す。

- recovery 完了後の状態
- persist event(s) 成功通知
- snapshot 成功通知
- snapshot delete 成功通知
- persistence failure

effector wrapper は recovery 中と persist 中に外部 command を stash し、永続化完了後に unstash する。stash mailbox は `PersistenceEffector::props(config, on_ready) -> TypedProps<M>` helper が `TypedProps::with_stash_mailbox()` を適用して満たす。低レベルの `from_config` から直接 `Behavior<M>` を組み立てる場合だけ、caller が同等の stash mailbox props を明示する。

### 3. 3 つの persistence mode を定義する

- `Persisted`: journal / snapshot store に書き込む本番用 mode
- `Ephemeral`: actor-system extension が所有する in-memory store に書き込む開発・テスト用 mode
- `Deferred`: 永続化を行わず callback だけ実行する dry-run / performance test 用 mode

`Persisted` / `Ephemeral` / `Deferred` は同じ `PersistenceEffector` API を使う。したがって、利用者は設定変更だけで段階的に移行できる。

### 4. EventSourcedBehavior 相当の API は non-goal にする

本 change では `EventSourcedBehavior` / `Effect` / `ReplyEffect` を公開 API として導入しない。typed persistence の第一の推奨 API は effector pattern とし、Pekko typed API の完全互換 wrapper が必要なら別 change で検討する。

## Capabilities

### New Capabilities

- **`persistence-effector-typed-api`**:
  - typed actor が通常の `Behavior<M>` として実装されたまま、event persistence / recovery / snapshot / retention を使える
  - ドメイン操作は `Result<NewState, Event>` またはドメイン固有 result を返し、command handler は新状態を clone せず one-shot callback に move して次の `Behavior` に渡せる
  - recovery は `apply_event(&S, &E) -> S` によって state を復元し、復元後に `(state, effector)` を `on_ready` に渡す
  - persist 中の command stashing、snapshot criteria、retention criteria、backoff restart を typed actor 側から設定できる
  - closed Rust enum の制約に合わせ、内部 store reply ではなく安定した `PersistenceEffectorSignal` を actor-private message へ包む adapter を提供する

### Modified Capabilities

- **`persistence-gap-analysis`**:
  - typed write-side API の方針を「Pekko `EventSourcedBehavior` 直接移植」から「effector-first typed persistence」に更新する
  - gap-analysis の typed persistence hard gap を、本 change の完了後に effector pattern 実装済みとして再分類する
  - persistence core の参照先を `modules/persistence-core-kernel` / `fraktor-persistence-core-kernel-rs` に更新する

## Impact

**影響を受けるコード:**

- workspace root `Cargo.toml`
- `modules/persistence-core/` → `modules/persistence-core-kernel/` rename
- `modules/persistence-core-kernel/Cargo.toml`
- `modules/persistence-core-kernel/src/**`
- `modules/persistence-core-typed/Cargo.toml` 新規
- `modules/persistence-core-typed/src/*.rs` 新規
- `modules/persistence-core-typed/src/internal/*.rs` 新規
- `modules/persistence-core-typed/tests/*` typed persistence integration tests 新規
- downstream `fraktor-persistence-core-rs` references の `fraktor-persistence-core-kernel-rs` への更新
- `docs/gap-analysis/persistence-gap-analysis.md`
- 必要に応じて `showcases/std/typed/persistence_effector/` 追加

**公開 API 影響:**

- typed effector API の新規追加に加え、既存 `fraktor-persistence-core-rs` の kernel rename を含む破壊的変更を行う。正式リリース前なので、classic persistence の crate 名・内部型整理は本 change の範囲に含める。
- `PersistenceEffector` 名は `Effector` サフィックスを含むが、`pekko-persistence-effector` 由来の外部参照語彙として採用する。
- `fraktor-persistence-core-rs` は `fraktor-persistence-core-kernel-rs` へ rename する。typed effector 利用者は `fraktor-persistence-core-typed-rs` を直接依存に追加する。

**挙動影響:**

- typed actor が `EventSourcedBehavior` なしで persistence を使える
- recovery 完了前と persist 中の外部 command は stash される
- domain validation error と persistence failure は明確に分離される

## Non-goals

- Pekko typed `EventSourcedBehavior` / `Effect` / `ReplyEffect` の 1:1 移植
- typed `DurableStateBehavior` の実装
- persistence-query
- storage backend 固有 plugin 実装
- JVM / HOCON / reflection 相当の plugin loading
- Java / Scala DSL convenience の再現
