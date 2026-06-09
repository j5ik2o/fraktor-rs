# Gap Analysis

## Summary

- **Feature**: `configure-cluster-failure-detector`
- **Discovery Scope**: Extension / Cluster Configuration (クラスタ設定) contract
- **Requirements Status**: generated / not approved
- **Key Findings**:
  - Failure Detector (故障検出器) の trait、registry、Phi Accrual 実装は存在するが、Cluster Configuration (クラスタ設定) から Failure Detector Configuration (故障検出器設定) を保持・検証・生成へ渡す contract がない。
  - Join Compatibility (参加互換性) の確認経路と key catalog は存在するため、Compatibility Mismatch Reason (互換性不一致理由) へ `cluster.failure-detector` を追加する拡張点はある。
  - 現状の `MembershipCoordinatorConfig` は `phi_threshold` と Membership Coordination Policy (メンバーシップ調停ポリシー) の timeout 系を同じ型に持っており、要求上の責務分離とずれている。
  - 既存テストには複数の Phi Accrual default 値があり、どれを cluster の既存 Availability Evidence (可用性観測証拠) として固定するかは設計フェーズで明示する必要がある。

## Current State Investigation

### Domain assets

- `modules/cluster-core-kernel/src/failure_detector/`
  - `FailureDetector` trait と `FailureDetectorRegistry` trait がある。
  - `DefaultFailureDetectorRegistry` は boxed factory から detector を生成できる。
  - Missing: `FailureDetectorConfig` と validation error はない。
- `modules/remote-core/src/failure_detector/phi_accrual.rs`
  - `PhiAccrualFailureDetector::new(address, threshold, max_sample_size, min_std_deviation, acceptable_heartbeat_pause, first_heartbeat_estimate)` がある。
  - 各観測パラメータの getter がある。
  - Constraint: Duration を public な Cluster Configuration (クラスタ設定) に置く場合、remote-core 側の primitive 値へ変換する境界が必要。
- `modules/cluster-core-kernel/src/extension/cluster_extension_config.rs`
  - `ClusterExtensionConfig` は pubsub、downing provider、role、static topology などを保持する。
  - `check_join_compatibility` と `JOIN_COMPATIBILITY_CHECKS` に Join Compatibility (参加互換性) の既存拡張点がある。
  - Missing: Failure Detector Configuration (故障検出器設定) の field、builder、validation、compatibility check がない。
- `modules/cluster-core-kernel/src/topology/cluster_compatibility_key_catalog.rs`
  - required key と excluded key の catalog がある。
  - `cluster.failure-detector.choice` は excluded key として存在する。
  - Missing: `cluster.failure-detector` の required key がない。
- `modules/cluster-core-kernel/src/membership/membership_coordinator_config.rs`
  - `phi_threshold`、`suspect_timeout`、`dead_timeout`、`quarantine_ttl`、`gossip_interval` が同居している。
  - Constraint: 要件では phi threshold は Failure Detector Configuration (故障検出器設定)、timeout/gossip 系は Membership Coordination Policy (メンバーシップ調停ポリシー) 側に残す。
- `docs/gap-analysis/cluster-gap-analysis.md`
  - Failure Detector (故障検出器) の implementation choice config が partial として残っている。
  - Constraint: 今回は Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) を実装しない。

### Conventions

- core crate は no_std 境界を守り、std 依存や actor 実行を持たない。
- std adaptor は core contract を実行へ接続する。
- public type は既存ルールに合わせて小さな sibling file に分ける。
- tests は sibling `tests.rs` や module-local test に置く傾向がある。
- domain docs は `CONTEXT.md` の用語に合わせ、初出で `English Term (日本語名)` を使う。

### Integration surfaces

- Cluster Configuration (クラスタ設定): `ClusterExtensionConfig`
- Join Compatibility (参加互換性): `ClusterExtensionConfig::check_join_compatibility`
- compatibility key catalog: `ClusterCompatibilityKeyCatalog`
- Availability Evidence (可用性観測証拠) の生成経路: `DefaultFailureDetectorRegistry` と Phi Accrual factory
- validation reporting: existing `ConfigValidation` は compatibility 用であり、Cluster Configuration Validation (クラスタ設定検証) とは分けて設計する必要がある。

## Requirement-to-Asset Map

| Requirement | Existing Assets | Gap |
| --- | --- | --- |
| 1. Failure Detector Configuration (故障検出器設定) を Cluster Configuration (クラスタ設定) に保持する | `ClusterExtensionConfig` | Missing: `FailureDetectorConfig` 型、field、default、builder |
| 1. Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) を公開契約にしない | `ClusterCompatibilityKeyCatalog::FAILURE_DETECTOR_CHOICE` excluded | Constraint: `cluster.failure-detector.choice` を required にしない guard test が必要 |
| 2. Phi Accrual 観測パラメータを区別する | `PhiAccrualFailureDetector` constructor/getter | Missing: Cluster Configuration (クラスタ設定) 上の typed config と Duration 境界 |
| 2. suspect/dead/quarantine/gossip を含めない | `MembershipCoordinatorConfig` に timeout/gossip 系が存在 | Constraint: 移動や削除ではなく責務境界を明示する設計が必要 |
| 3. Cluster Configuration Validation (クラスタ設定検証) | 互換性用 `ConfigValidation` のみ | Missing: install/start 前に呼ぶ validation API と `FailureDetectorConfigError` |
| 4. Join Compatibility (参加互換性) に含める | `check_join_compatibility`、`JOIN_COMPATIBILITY_CHECKS` | Missing: `cluster.failure-detector` key と field-level diff reason |
| 5. std 環境で Availability Evidence (可用性観測証拠) へ反映する | `DefaultFailureDetectorRegistry`、Phi Accrual adapter test patterns | Unknown: production path の detector factory 接続点 |
| 6. gap analysis docs を完了扱いに更新する | `docs/gap-analysis/cluster-gap-analysis.md` | Missing: 実装完了後の該当項目更新 |

## Implementation Approach Options

### Option A: Extend Existing Components

`ClusterExtensionConfig`、`ClusterCompatibilityKeyCatalog`、`ClusterExtensionConfig::check_join_compatibility` を直接拡張し、Failure Detector Configuration (故障検出器設定) の値・validation・Compatibility Mismatch Reason (互換性不一致理由) を既存 file に追加する。

- **Files/modules to extend**:
  - `modules/cluster-core-kernel/src/extension/cluster_extension_config.rs`
  - `modules/cluster-core-kernel/src/topology/cluster_compatibility_key_catalog.rs`
  - existing cluster-core tests
- **Strengths**:
  - 変更箇所が少なく、既存 Join Compatibility (参加互換性) の流れに乗せやすい。
  - backward compatibility の確認対象が限定される。
- **Risks / Limitations**:
  - `ClusterExtensionConfig` が validation と field diff formatting まで抱え、責務が増える。
  - `FailureDetectorConfigError` の置き場所が曖昧になりやすい。
  - default 値の根拠が file 内で見えにくくなる。

### Option B: Create New Components

`FailureDetectorConfig`、`FailureDetectorConfigError`、差分計算 helper を Failure Detector (故障検出器) 側の独立 component として追加し、既存 config と compatibility catalog は integration のみを持つ。

- **Files/modules to create**:
  - `modules/cluster-core-kernel/src/failure_detector/failure_detector_config.rs`
  - `modules/cluster-core-kernel/src/failure_detector/failure_detector_config_error.rs`
  - 必要に応じて config diff helper
- **Strengths**:
  - Failure Detector Configuration (故障検出器設定) の責務が明確になり、単体 test を書きやすい。
  - Cluster Configuration Validation (クラスタ設定検証) の error taxonomy を config 型の近くに置ける。
- **Risks / Limitations**:
  - 新規 file と export が増える。
  - Join Compatibility (参加互換性) との接続設計を誤ると、config 型が compatibility policy を持ちすぎる。
  - std 側の factory 接続までは別途 integration が必要。

### Option C: Hybrid Approach

Failure Detector Configuration (故障検出器設定) の型と validation は新 component にし、Cluster Configuration (クラスタ設定) と Join Compatibility (参加互換性) の接続だけを既存 component へ最小追加する。

- **Combination strategy**:
  - `FailureDetectorConfig` と `FailureDetectorConfigError` は `failure_detector` module に置く。
  - `ClusterExtensionConfig` は field、default、`with_failure_detector_config`、validation 呼び出しだけを持つ。
  - `ClusterCompatibilityKeyCatalog` に `cluster.failure-detector` を追加し、`cluster.failure-detector.choice` は excluded のまま保つ。
  - std adaptor は Cluster Configuration (クラスタ設定) から Phi Accrual factory を作る境界を持つ。
- **Strengths**:
  - core/adaptor 分離と one-public-type-per-file の既存規約に合う。
  - Failure Detector Configuration (故障検出器設定) と Join Compatibility (参加互換性) の責務を分けられる。
  - 設計フェーズで default と validation の根拠を明示しやすい。
- **Risks / Limitations**:
  - 複数 module を触るため、test plan と migration order が必要。
  - production path の detector factory 接続点が未確認の場合、設計で追加調査が必要。

## Complexity & Risk

- **Effort**: M
  - core config、validation、Join Compatibility (参加互換性)、std factory 接続、docs 更新、tests にまたがるが、既存の拡張点は明確。
- **Risk**: Medium
  - default 値の canonical source と production factory 接続点に不明点がある。Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) へ scope が広がるリスクも guard が必要。

## Recommendations for Design Phase

- **Preferred approach**: Option C を第一候補にする。
  - `FailureDetectorConfig` は Availability Evidence (可用性観測証拠) の観測パラメータだけを所有する。
  - `ClusterExtensionConfig` は Cluster Configuration (クラスタ設定) の aggregate として保持・validation 呼び出し・Join Compatibility (参加互換性) への接続を担う。
  - `PhiAccrualFailureDetector` の selection は固定し、`cluster.failure-detector.choice` を required key にしない。
- **Key decisions to make**:
  - canonical default values をどの既存 cluster path から採るか。
  - `FailureDetectorConfigError` の variant 名と表示文をどうするか。
  - validation を install/start boundary のどの public API で強制するか。
  - `required_join_compatibility_keys()` の field-level metadata と `ClusterCompatibilityKeyCatalog` の single key をどう整合させるか。
  - std adaptor の production factory 接続点をどこに置くか。

## Research Needed

- Existing cluster-path default の特定:
  - cluster-adaptor tests は `threshold, 10, 1, 0, 10` 系を使う。
  - cluster-core registry test は `1.5, 4, 1, 0, 10` を使う。
  - remote watcher の `5.0, 100, 10, 0, 100` は remote-core 側の用途であり、cluster default と混同しない確認が必要。
- Production detector factory path:
  - 現状は tests で `DefaultFailureDetectorRegistry` を組み立てる箇所が目立つ。
  - std 環境で Cluster Configuration (クラスタ設定) から Availability Evidence (可用性観測証拠) へ流す production path を設計前に特定する。
- Validation boundary:
  - builder は不変式を強制せず、install/start で Cluster Configuration Validation (クラスタ設定検証) を実行する要求がある。
  - 実際に install/start を担う API と error propagation を確認する。
- Join Compatibility (参加互換性) metadata:
  - 要件は single key `cluster.failure-detector` を Compatibility Mismatch Reason (互換性不一致理由) に出す。
  - 既存 `required_join_compatibility_keys()` は field-level `fraktor.*` strings を返しているため、公開情報として残すか整理するかを設計で決める。

## Boundary Guards

- Failure Detector Configuration (故障検出器設定) は Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) を含めない。
- `cluster.failure-detector.choice` は required key にしない。
- Failure Detector (故障検出器) に Downing Decision (ダウン判断) や Member Removal (メンバー除去) の責務を追加しない。
- Split Brain Resolver execution actor、provider down execution loop、lease coordination backend はこの feature に含めない。
- Cluster Singleton、Cluster Client、Receptionist、Distributed Data / CRDT、Pekko public API parity はこの feature に含めない。
- `cluster-core-kernel` に std 依存を入れない。

## References

- `.kiro/specs/configure-cluster-failure-detector/requirements.md` — generated requirements.
- `CONTEXT.md` — authoritative domain glossary.
- `docs/adr/0001-failure-detector-configuration-contract.md` — Failure Detector Configuration (故障検出器設定) の contract decision.
- `docs/gap-analysis/cluster-gap-analysis.md` — source gap.
- `modules/cluster-core-kernel/src/extension/cluster_extension_config.rs` — Cluster Configuration (クラスタ設定) と Join Compatibility (参加互換性) の既存実装。
- `modules/cluster-core-kernel/src/topology/cluster_compatibility_key_catalog.rs` — compatibility key catalog.
- `modules/cluster-core-kernel/src/failure_detector/default_failure_detector_registry.rs` — detector factory registry.
- `modules/remote-core/src/failure_detector/phi_accrual.rs` — Phi Accrual detector implementation.

---

# Design Discovery & Decisions

## Summary

- **Feature**: `configure-cluster-failure-detector`
- **Discovery Scope**: Extension / light discovery
- **Key Findings**:
  - `ClusterExtensionConfig` と `ClusterCompatibilityKeyCatalog` は Join Compatibility (参加互換性) の拡張点を既に持つため、既存 component へ最小接続する設計で足りる。
  - `ClusterExtensionInstaller::install` と `ClusterCore::start_member` / `start_client` が Cluster Configuration Validation (クラスタ設定検証) の自然な境界になる。
  - std 側の Availability Evidence (可用性観測証拠) 反映は、新しい外部依存ではなく `PhiAccrualFailureDetector` を作る小さな factory helper で実現できる。

## Research Log

### Validation boundary

- **Context**: 要件 3.1 は install / start 境界で Failure Detector Configuration (故障検出器設定) を検証することを求める。
- **Sources Consulted**:
  - `modules/cluster-core-kernel/src/extension/cluster_extension_installer.rs`
  - `modules/cluster-core-kernel/src/extension/cluster_core.rs`
  - `modules/cluster-core-kernel/src/extension/cluster_error.rs`
- **Findings**:
  - installer は `ActorSystemBuildError::Configuration(String)` へ設定失敗を写せる。
  - start member / client は `ClusterError` を返すため、config validation failure を cluster lifecycle error として返せる。
  - `ClusterCore` は現状 config 全体を保持しないが、start 境界で検証するには Failure Detector Configuration (故障検出器設定) を保持する必要がある。
- **Implications**:
  - `ClusterExtensionConfig::validate()` を install 前に呼ぶ。
  - `ClusterCore` は `FailureDetectorConfig` の clone を保持し、start member / client の最初に検証する。
  - builder API では validation を強制しない。

### Existing default values

- **Context**: 要件 1.2 は既存 Availability Evidence (可用性観測証拠) の観測挙動と同等の default を求める。
- **Sources Consulted**:
  - `modules/cluster-core-kernel/src/membership/membership_coordinator_test.rs`
  - `modules/cluster-adaptor-std/src/membership/tokio_gossiper_test.rs`
  - `modules/cluster-adaptor-std/tests/gossip_tokio_integration.rs`
  - `modules/remote-core/src/watcher/watcher_state.rs`
- **Findings**:
  - cluster membership path は `phi_threshold = 1.0`、`max_sample_size = 10`、`min_std_deviation = 1ms`、`acceptable_heartbeat_pause = 0ms`、`first_heartbeat_estimate = 10ms` を繰り返し使う。
  - remote watcher path は `5.0, 100, 10ms, 0ms, 100ms` を使うが、remote-core 側の watcher 用であり cluster default とは別の用途である。
- **Implications**:
  - `FailureDetectorConfig::new()` は cluster membership path の値を default とする。
  - remote watcher default はこの spec の default source にしない。

### Join Compatibility metadata

- **Context**: 要件 4.4 と 4.5 は single key `cluster.failure-detector` と差分のある観測パラメータ名を求める。
- **Sources Consulted**:
  - `modules/cluster-core-kernel/src/extension/cluster_extension_config.rs`
  - `modules/cluster-core-kernel/src/topology/cluster_compatibility_key_catalog.rs`
- **Findings**:
  - compatibility reason は catalog key と detail string を結合する既存 helper で生成される。
  - `cluster.failure-detector.choice` は excluded key として既に定義されている。
  - `required_join_compatibility_keys()` は field-level `fraktor.*` metadata strings を返す既存 API で、catalog の stable key と粒度が異なる。
- **Implications**:
  - catalog へ required key `cluster.failure-detector` を追加する。
  - Failure Detector Configuration (故障検出器設定) は field-level metadata key を増やさず、required metadata も `cluster.failure-detector` に寄せる。Compatibility Mismatch Reason (互換性不一致理由) は同じ key に固定し、detail へ field 名だけを含める。
  - `cluster.failure-detector.choice` は required / field-level metadata に追加しない。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Extend existing components only | `ClusterExtensionConfig` に型・validation・diff を直接追加する | 変更箇所が少ない | config が責務過多になりやすい | 不採用 |
| New components only | Failure Detector (故障検出器) 側に config と compatibility を閉じる | 単体責務は明確 | Cluster Configuration (クラスタ設定) との接続が遠くなる | 不採用 |
| Hybrid | config/error は新 component、既存 config/catalog/core/std helper は接続だけ持つ | core/adaptor 境界と one-public-type-per-file に合う | 複数 file にまたがる | 採用 |

## Design Decisions

### Decision: Failure Detector Configuration (故障検出器設定) は focused value object にする

- **Context**: 観測パラメータと Membership Coordination Policy (メンバーシップ調停ポリシー) を分離する必要がある。
- **Alternatives Considered**:
  1. `MembershipCoordinatorConfig` の `phi_threshold` 周辺を拡張する。
  2. `ClusterExtensionConfig` に field を直接並べる。
  3. `FailureDetectorConfig` を独立 value object にする。
- **Selected Approach**: `FailureDetectorConfig` を `failure_detector` module に置き、Phi Accrual 前提の観測パラメータだけを所有させる。
- **Rationale**: `CONTEXT.md` の用語境界と一致し、timeout/gossip policy を混入させない。
- **Trade-offs**: `ClusterExtensionConfig` と std helper への接続が必要になる。
- **Follow-up**: `MembershipCoordinatorConfig::phi_threshold` の扱いは実装で互換性を壊さない範囲に留める。

### Decision: validation は builder ではなく install / start 境界で実行する

- **Context**: 要件は Cluster Configuration Validation (クラスタ設定検証) を install / start 境界で扱うことを求める。
- **Alternatives Considered**:
  1. `with_*` builder で即時 reject する。
  2. `ClusterExtensionConfig::validate()` を明示し、installer と core start で呼ぶ。
- **Selected Approach**: builder は値保持のみ、`validate()` を install / start の先頭で呼ぶ。
- **Rationale**: 既存 builder style を維持し、設定値そのものの誤りを Join Compatibility (参加互換性) より前に返せる。
- **Trade-offs**: 無効な config value を一時的に保持できるため、start/install tests が必要。
- **Follow-up**: install error は `ActorSystemBuildError::Configuration`、start error は `ClusterError::Configuration` 系へ写す。

### Decision: std bridge は Phi Accrual helper に閉じる

- **Context**: std 環境では Cluster Configuration (クラスタ設定) の値を Availability Evidence (可用性観測証拠) の観測に渡す必要がある。
- **Alternatives Considered**:
  1. cluster-core が `PhiAccrualFailureDetector` を直接生成する。
  2. std adaptor に helper を置いて remote-core の detector へ変換する。
- **Selected Approach**: std adaptor に `ConfiguredPhiAccrualDetectorFactory` を追加し、`FailureDetectorConfig` から `PhiAccrualFailureDetector` を生成する。
- **Rationale**: cluster-core に concrete detector 依存を増やさず、std 側で remote-core 実装を使える。
- **Trade-offs**: helper は algorithm selection surface ではなく固定 bridge であることを docs と tests で guard する。
- **Follow-up**: production assembly が helper を使う箇所は実装タスクで既存 factory flow に沿って接続する。

## Risks & Mitigations

- default 値が意図せず remote watcher default に寄る — cluster membership tests の既存値を `FailureDetectorConfig::new()` の source として固定する。
- Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) へ scope が広がる — `cluster.failure-detector.choice` を excluded のままにし、std helper 名も choice を示さない。
- validation failure と Join Compatibility (参加互換性) failure が混ざる — invalid config tests と mismatch config tests を分ける。

## References

- `CONTEXT.md` — domain terminology.
- `docs/adr/0001-failure-detector-configuration-contract.md` — contract decision.
- `.kiro/specs/configure-cluster-failure-detector/requirements.md` — accepted requirements for this design.
- `modules/cluster-core-kernel/src/extension/cluster_extension_config.rs` — Cluster Configuration (クラスタ設定) aggregate and Join Compatibility (参加互換性).
- `modules/cluster-core-kernel/src/extension/cluster_core.rs` — start member / client validation boundary.
- `modules/cluster-adaptor-std/src/membership/tokio_gossiper_test.rs` — existing cluster-path detector parameters.
