## Context

`fraktor-cluster-core-rs` はまだ未リリースなので、module path compatibility を package structure の制約にしない。現在の `modules/cluster-core/src` には、肥大化した crate root と広すぎる sibling module がある。

- root-level files が extension wiring、provider handle、topology event、config validation、metrics、request/resolve errors を混在させている。
- `identity`、`placement`、`grain` の一部は、分散 Grain activation を中心に同時変更されている。
- `grain` には、activation 固有ではない Grain API、context、RPC、codec、serialization、metrics も含まれている。
- `membership`、`failure_detector`、`downing_provider`、`pub_sub`、`outbound` は、すでに別々の変更理由を表現している。

この設計では core/adaptor の依存方向を維持する。cluster-core が port / policy を所有し、std adapter はこの crate の外側で host-specific behavior を実装する。

## Goals / Non-Goals

**Goals:**

- `cluster-core` の module tree が主要な変更境界を表す状態にする。
- 分散 Grain activation と ownership resolution の境界として `activation` を導入する。
- Grain-facing API と messaging concerns は `grain` に残す。
- root-level cluster files を `extension` と `topology` に分ける。
- 旧 submodule path compatibility を残さず、互換 shim を追加しない。
- `runtime`、`manager`、`service`、`engine` のような曖昧な module 名を避ける。

**Non-Goals:**

- cluster behavior、activation algorithm、gossip behavior、downing decision、pub/sub semantics、outbound delivery semantics は変更しない。
- `cluster-grain-core` のような新 crate は作らない。
- std-specific adapter behavior を `cluster-core` に移さない。
- `membership`、`failure_detector`、`downing_provider` を統合しない。
- `pub_sub` と `outbound` を統合しない。

## Decisions

### Decision 1: identity と placement の上位境界は `activation` にする

identity lookup、PID cache、rendezvous hashing、placement coordination、activation records、activation storage、leases、locks、command results、virtual actor activation registry を `activation` 配下へ移動する。

この名前は、これらの files が一緒に変更される理由、つまり Grain key の owner node を決め、activation を成立させる責務を表す。`runtime` は何が動くのかを示さず、local naming rule でも禁止されているため採用しない。

### Decision 2: `grain` は caller-facing Grain and RPC API に集中させる

`GrainKey`、`GrainRef`、`GrainContext`、call options、codec、serialized message types、RPC router / dispatch / events / errors、schema negotiation、Grain metrics は `grain` に残す。

`VirtualActorRegistry` と `VirtualActorEvent` は、caller-facing Grain interaction ではなく activation ownership と cache invalidation を扱うため `activation` に移す。

### Decision 3: root-level cluster files は `extension` と `topology` に分ける

`extension` は ActorSystem integration と cluster entrypoints を扱う。

- `ClusterApi`
- `ClusterCore`
- `ClusterExtension`
- `ClusterExtensionConfig`
- `ClusterExtensionId`
- `ClusterExtensionInstaller`
- `ClusterProviderShared`
- startup mode
- extension flow に属する request / resolve / extension / cluster / provider / API / metrics errors

`topology` は observed cluster state と topology-change contracts を扱う。

- `BlockListProvider`
- `ClusterEvent`
- `ClusterEventType`
- `ClusterMetrics`
- `ClusterMetricsSnapshot`
- `ClusterTopology`
- `ConfigValidation`
- `JoinConfigCompatChecker`
- `TopologyApplyError`
- `TopologyUpdate`

`cluster` module を作る案は採用しない。crate 自体が cluster boundary であり、`cluster` だけでは変更理由を識別できないため。

### Decision 4: 既存の独立境界は並列維持する

以下は top-level peer modules として維持する。

- `membership`: gossip、member table、member lifecycle、quarantine、vector clock。
- `failure_detector`: suspicion detection ports and registry。
- `downing_provider`: failure observations and downing policy decisions。
- `pub_sub`: cluster-wide publish/subscribe contracts and broker。
- `outbound`: outbound envelope、action、state、delivery pipeline。
- `cluster_provider`: cluster provider port と local/static/no-op implementations。

これらは extension、topology、activation と連携するが、primary change reason は別である。

### Decision 5: 旧 public module paths は意図的に壊す

旧 path を守るためだけの `identity`、`placement`、root-level forwarding modules は追加しない。実装では ergonomics のために selected crate-root re-exports を維持できるが、それは compatibility alias ではなく canonical public API として意図する場合に限る。

## Risks / Trade-offs

- Large file moves が behavior change を隠す -> implementation commits はまず mechanical move に限定し、追加 cleanup の前に focused compile/tests を実行する。
- Broad root re-exports が新しい境界を曖昧にする -> move 後に `lib.rs` を監査し、意図した public entrypoints だけ残す。
- `activation` が Grain concerns 全部を吸い込む -> RPC、codec、context、metrics は `grain` に残し、tests/import review で分割を守る。
- Intra-crate imports が移動中に乱れる -> boundary ごとに小さく module declarations と imports を更新する。

## Migration Plan

1. 新しい `activation`、`extension`、`topology` module files/directories を作る。
2. identity、placement、activation registry files を `activation` に移動し、compatibility shim modules は作らない。
3. Grain-facing API/RPC/codec files は `grain` に残し、移動した activation types への imports を更新する。
4. root extension and topology files を `extension` と `topology` に移動する。
5. `lib.rs` の module declarations と intentional public re-exports を更新する。
6. sibling tests と intra-crate imports を更新する。
7. `openspec validate reorganize-cluster-core-modules --strict`、targeted `fraktor-cluster-core-rs` tests、`cargo fmt --check --all`、`git diff --check` を実行する。

Rollback は implementation commits の revert で行う。crate は未リリースなので compatibility migration period は設けない。

## Open Questions

- どの crate-root re-exports を canonical entrypoints として残すかは、実装時に current examples と package-level docs を監査して決める。
