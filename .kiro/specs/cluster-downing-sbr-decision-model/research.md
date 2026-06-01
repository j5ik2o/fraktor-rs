# Research & Design Decisions

## Summary

- **Feature**: `cluster-downing-sbr-decision-model`
- **Discovery Scope**: Extension / Complex Integration
- **Key Findings**:
  - `downing_provider` には `DowningInput`、`DowningDecision`、`DowningProviderCompatibility`、`SplitBrainResolverSettings`、`SplitBrainResolverStrategy`、`NoopDowningProvider` が存在し、decision model の土台はある。
  - upstream `cluster-membership-reachability-model` は `UniqueAddress`、data center、`WeaklyUp`、`ReachabilityMatrix`、`IndirectConnectionEvidence` を downing input として公開する前提を持つ。
  - lease majority は core に host I/O を持ち込まず、core は lease acquisition の port contract と decision semantics、std/provider は backend binding と lifecycle cleanup を担当するのが最小境界である。

## Research Log

### 既存 downing provider surface
- **Context**: downing/SBR decision model をどこへ接続するか確認した。
- **Sources Consulted**: `modules/cluster-core-kernel/src/downing_provider/*.rs`
- **Findings**:
  - `DowningProvider` trait は `decide(&mut self, input: &DowningInput)` を持つ。
  - `DowningInput` は explicit down と `FailureObservation` に限定されている。
  - `SplitBrainResolverSettings` と `SplitBrainResolverStrategy` は compatibility metadata として存在するが、strategy evaluation はまだ持たない。
- **Implications**: 既存 trait を置き換えるより、SBR evaluator と richer decision context を `downing_provider` 内へ追加し、provider hook はそれを呼び出す設計にする。

### upstream membership alignment
- **Context**: SBR が参照する membership evidence の所有境界を確認した。
- **Sources Consulted**: `.kiro/specs/cluster-membership-reachability-model/design.md`
- **Findings**:
  - membership spec は `UniqueAddress`、data center、`WeaklyUp`、`ReachabilityMatrix`、`IndirectConnectionEvidence` を core/membership が所有する。
  - downing は reachability evidence を入力として受け取るが、matrix や heartbeat は所有しない。
- **Implications**: 本 spec は reachability structure を再定義せず、SBR input snapshot と evaluation expectation だけを定義する。

### compatibility baseline alignment
- **Context**: provider-facing SBR integration と compatibility metadata の前提を確認した。
- **Sources Consulted**: `.kiro/specs/cluster-active-compatibility-baseline/design.md`
- **Findings**:
  - baseline spec は SBR provider-facing hook、provider key、settings identity を扱う。
  - full decision actor、strategy evaluator、lease abstraction は baseline の out of boundary である。
- **Implications**: 本 spec は baseline の provider hook を拡張し、decision evaluator と lease majority port を追加する。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| core evaluator + std lease binding | core に SBR decision semantics と lease port、std に lease backend binding を置く | `no_std` 境界と provider lifecycle が明確 | port contract の初期設計が必要 | 採用 |
| provider 実装ごとに strategy を実装 | 各 provider が DowningProvider 内で strategy を直接評価する | 初期実装は少ない | decision semantics が重複し、review scope が崩れる | 却下 |
| membership module に downing decision を置く | reachability matrix と同じ場所で downing を判断する | data access は近い | membership が policy を吸収し、downing spec の境界が消える | 却下 |

## Design Decisions

### Decision: SBR evaluator は core/downing_provider が所有する
- **Context**: strategy identity は既に core に存在し、provider は lifecycle と hook を持つ。
- **Alternatives Considered**:
  1. provider ごとに evaluator を実装する。
  2. membership が reachability aggregate と同時に downing decision を返す。
  3. core/downing_provider に evaluator を置く。
- **Selected Approach**: core/downing_provider に `SplitBrainResolver` evaluator と `DowningStrategyDecision` contract を追加する。
- **Rationale**: decision semantics を一箇所に置き、std/provider は host binding に限定できる。
- **Trade-offs**: upstream membership snapshot の shape が変わると revalidation が必要になる。
- **Follow-up**: implementation 時に `cluster-membership-reachability-model` の final type 名と一致させる。

### Decision: lease majority は port contract と result vocabulary だけを core に置く
- **Context**: lease acquisition は network や external service に接続し得る。
- **Alternatives Considered**:
  1. core に lease backend 実装を置く。
  2. std/provider に decision semantics まで置く。
  3. core は lease port と result vocabulary、std は backend binding を持つ。
- **Selected Approach**: core は `LeaseMajorityPort` と `LeaseAcquisitionOutcome` を定義し、SBR evaluator は outcome を decision trace に反映する。
- **Rationale**: `no_std` core を保ちつつ lease-based majority の semantics を検証できる。
- **Trade-offs**: std adaptor に最低1つの test backend が必要になる。
- **Follow-up**: lease timeout と retry policy は host binding 側で扱い、core は同期的な outcome 入力として評価する。

### Decision: downstream protocol work は明示的に吸収しない
- **Context**: roadmap には gossip heartbeat、discovery、pubsub、serialization が別 spec として並ぶ。
- **Alternatives Considered**:
  1. SBR に必要な範囲で heartbeat や gossip を同時に設計する。
  2. SBR input から protocol への期待を完全に隠す。
- **Selected Approach**: SBR は upstream membership evidence に依存し、protocol generation や wire contract は扱わない。
- **Rationale**: downing policy と transport protocol の PR scope を分けられる。
- **Trade-offs**: implementation 順では upstream spec の型名確定を待つ必要がある。
- **Follow-up**: cross-spec review で downstream spec が SBR input を逆方向に拡張していないか確認する。

## Risks & Mitigations

- reachability type name の drift — implementation 前に upstream membership spec の generated design と実装差分を確認する。
- SBR evaluator の過剰抽象化 — current strategies に必要な decision vocabulary だけを定義し、actor や scheduler を作らない。
- lease backend が core に漏れる — core は port と outcome に限定し、std/provider task に backend binding を集約する。
- provider lifecycle と pending lease operation の cleanup 漏れ — std adaptor task で lifecycle stop/drop 時の cleanup test を必須にする。

## References

- `.kiro/specs/cluster-downing-sbr-decision-model/brief.md` — discovery scope
- `.kiro/specs/cluster-active-compatibility-baseline/design.md` — provider-facing baseline
- `.kiro/specs/cluster-membership-reachability-model/design.md` — membership / reachability input boundary
- `modules/cluster-core-kernel/src/downing_provider/*.rs` — existing downing surface
