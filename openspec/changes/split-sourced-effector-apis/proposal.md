## Why

現在の typed `PersistenceEffector` は名前上は persistence 全般に見えるが、実体は event journal、snapshot、`apply_event` に基づく Event Sourcing 専用 API である。State Sourcing 用の Effector を追加する前に、Event Sourcing と State Sourcing を兄弟 API として分離し、typed persistence の public contract を明確にする。

## What Changes

- **BREAKING**: `PersistenceEffector*` public API を `EventSourcedEffector*` へ改名し、互換 alias / deprecated 経路は残さない。
- `EventSourcedEffector` は現行の event-sourced effector semantics を維持する。つまり recovery 後に `on_ready(state, effector)` が `Behavior<M>` を返し、persist callback も最後に次の `Behavior<M>` を返す。
- `StateSourcedEffector*` public API を追加し、durable state store の `get_object` / `upsert_object` / `delete_object` を typed actor から Effector スタイルで扱えるようにする。
- `StateSourcedEffector` は event replay や `apply_event` を要求せず、revision 付き durable state の recovery / persist / delete 結果を signal として user private message に包む。
- Pekko direct `EventSourcedBehavior` / `DurableStateBehavior` DSL と public `Effect` / `EffectBuilder` / `ReplyEffect` DSL は non-goal とし、Event Sourcing / State Sourcing とも Effector スタイルを正とする。
- 既存 `DurableStateSignal` は State Sourcing 用 signal として扱い、必要に応じて `StateSourcedEffectorSignal` へ命名を揃える。
- `docs/gap-analysis/persistence-gap-analysis.md` の typed Effector / durable state 行を、新しい API 名と実装範囲に合わせて更新する。

## Capabilities

### New Capabilities

- `state-sourced-effector-typed-api`: typed actor が Durable State / State Sourcing を Effector スタイルで使うための public API と runtime contract。

### Modified Capabilities

- `persistence-effector-typed-api`: 既存 Event Sourcing Effector API を `EventSourcedEffector*` として再定義し、`PersistenceEffector*` 名を public contract から削除する。

## Impact

- `modules/persistence-core-typed/src/`
- `modules/persistence-core-typed/tests/`
- `modules/persistence-core-kernel/src/state/`
- `openspec/specs/persistence-effector-typed-api/spec.md`
- `openspec/changes/split-sourced-effector-apis/specs/`
- `docs/gap-analysis/persistence-gap-analysis.md`

この change は正式リリース前の破壊的変更として扱い、旧 `PersistenceEffector*` 名の後方互換層は追加しない。
