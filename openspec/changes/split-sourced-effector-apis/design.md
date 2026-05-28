## Context

`modules/persistence-core-typed` は現在 `PersistenceEffector` を公開しているが、config は `E` と `apply_event(&S, &E) -> S` を必須とし、内部 store actor は journal / snapshot / event replay を前提にしている。これは Event Sourcing 用 API としては成立している一方、State Sourcing / Durable State 用 API を同じ `PersistenceEffector` 名の下へ追加すると、event journal と durable state object store の契約が混ざる。

kernel 側には `DurableStateStore`、`DurableStateStoreRegistry`、`GetObjectResult`、`DurableStateError` が存在し、typed 側にも将来の durable state integration 用 `DurableStateSignal` がある。ただし typed Effector runtime は durable state store に接続されていない。

この change は、Effector API を fraktor 独自の typed actor integration として整理する。Pekko 直接互換の `EventSourcedBehavior` / `DurableStateBehavior` DSL ではなく、`Behavior<M>` を返す Effector スタイルを Event Sourcing と State Sourcing の両方で提供する。

## Goals / Non-Goals

**Goals:**

- `PersistenceEffector*` を `EventSourcedEffector*` へ破壊的に改名し、event-sourced API であることを明確にする。
- 旧 `PersistenceEffector*` 名の alias、deprecated type、互換 module を残さない。
- `StateSourcedEffector*` を追加し、durable state object の recovery / persist / delete を Effector スタイルで扱えるようにする。
- Event Sourcing と State Sourcing の両方で、operation callback が最後に次の `Behavior<M>` を返す意味論を維持する。
- kernel / typed の no_std 境界を維持し、std 固有 runtime や filesystem store を追加しない。

**Non-Goals:**

- Pekko 互換の `EventSourcedBehavior` / `DurableStateBehavior` DSL を追加しない。
- Pekko typed `EffectBuilder` / `ReplyEffect` 互換 API を追加しない。
- event-sourced API と state-sourced API を一つの enum mode や umbrella `PersistenceEffector` に統合しない。
- durable state query / changes stream / projection runtime を追加しない。
- durable state store の backend 実装や storage plugin selection を追加しない。

## Decisions

### Decision 1: public Effector API は `EventSourcedEffector` と `StateSourcedEffector` の兄弟にする

Event Sourcing は event を永続化し、recovery で event replay と `apply_event` により state を復元する。State Sourcing は state object を永続化し、recovery で latest object と revision を読み戻す。必要な config、signal、internal command、failure handling が異なるため、ひとつの `PersistenceEffector` に mode として押し込まない。

代替案として `PersistenceEffector` を umbrella/facade として残す案があるが、`E` と `apply_event` を持たない State Sourcing 側に不要な型パラメータや設定を持ち込むため採用しない。

### Decision 2: rename は互換層なしで完了させる

この repository は正式リリース前であり、project 原則として legacy alias / deprecated 経路を残さない。`PersistenceEffector`、`PersistenceEffectorConfig`、`PersistenceEffectorSignal`、`PersistenceEffectorMessageAdapter`、`PersistenceEffectorSignalAuth` は canonical 名ではなくなるため、実装時に `EventSourcedEffector*` へ直接置き換える。

代替案として type alias を残す案があるが、State Sourcing 追加後に `PersistenceEffectorConfig` と `StateSourcedEffectorConfig` が同階層に見え、命名の MECE 性が崩れるため採用しない。

### Decision 3: `StateSourcedEffector` は durable state store contract を使う

State Sourcing 側は kernel の `DurableStateStore<S>` を使い、起動時に `get_object(persistence_id)` で `Option<S>` と revision を復元する。persist は `upsert_object(persistence_id, expected_revision, state, tag)` を呼び、成功後に revision を進めた signal を user private message へ届ける。delete は `delete_object(persistence_id, expected_revision)` を呼び、成功後に deleted revision を signal 化する。

代替案として event-sourced snapshot store を state object store として流用する案があるが、snapshot は event replay の最適化であり、durable state の expected revision / delete semantics と一致しないため採用しない。

### Decision 4: callback が次の `Behavior<M>` を返す意味論は維持する

Effector スタイルの価値は、command handler が domain operation の結果を受け取り、persistence 成功後に次の behavior を返して状態遷移する点にある。State Sourcing 側でも `persist_state` / `delete_state` の callback は one-shot で、保存完了後に `Behavior<M>` を返す。

代替案として state を effector 内部に閉じ込めて user handler から直接 mutate させる案があるが、typed actor の状態別 behavior と相性が悪く、現行 Event Sourcing Effector の設計意図ともずれるため採用しない。

### Decision 5: signal 命名は State Sourcing public API に揃える

既存の `DurableStateSignal` は durable state lifecycle signal として妥当だが、Effector API を `StateSourcedEffector` と呼ぶなら public signal 名も `StateSourcedEffectorSignal` へ揃える。kernel / storage contract の `DurableStateStore` 名は維持し、typed actor に見せる Effector API 名と storage contract 名を分離する。

代替案として `DurableStateSignal` をそのまま public signal にする案があるが、`StateSourcedEffector` が `DurableStateSignal` を返す形になり、typed Effector API の命名軸が混ざるため採用しない。

## Risks / Trade-offs

- Public rename の差分が大きい -> rename と State Sourcing 実装を task 上で分け、まず `EventSourcedEffector*` rename を compile / test で閉じる。
- State Sourcing runtime が event-sourced store actor の copy になりすぎる -> hidden actor / stash / signal adapter の構造は共有してよいが、store command と config は state-sourced 専用に分ける。
- revision 更新の off-by-one が入りやすい -> `GetObjectResult::revision()`、`upsert_object(expected_revision)`、success signal の revision を focused tests で固定する。
- durable state provider 解決の責務が曖昧になる -> config には provider id または store provider hook を明示し、typed crate が kernel registry contract を経由する。

## Migration Plan

1. `PersistenceEffector*` modules / types / tests / docs を `EventSourcedEffector*` へ rename し、旧名を削除する。
2. `StateSourcedEffectorConfig`、`StateSourcedEffectorSignal`、`StateSourcedEffectorMessageAdapter`、internal state store command / reply / actor を追加する。
3. State Sourcing recovery、persist、delete の callback / stash / failure semantics を実装する。
4. public-surface tests と focused flow tests を追加する。
5. `openspec validate split-sourced-effector-apis --strict`、targeted cargo tests、`cargo fmt --check --all`、`git diff --check` を実行する。

Rollback はこの change の実装 commit を revert する。正式リリース前のため旧 `PersistenceEffector*` 名へ戻す互換移行期間は設けない。

## Open Questions

- `StateSourcedEffectorConfig` が durable state provider を直接受けるか、provider id と registry lookup を受けるかは、既存 persistence extension の registry 所有場所を実装時に確認して決める。
- `StateSourcedEffectorSignal` の payload に persisted state を含めるか、revision だけにするかは、callback API の使い方と clone bound を見て最小契約にする。
