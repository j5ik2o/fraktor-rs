## ADDED Requirements

### Requirement: Activation boundary owns distributed Grain activation

`fraktor-cluster-core-rs` は分散 Grain activation の module boundary として `activation` を定義する SHALL。この boundary は identity lookup、PID cache、rendezvous hashing、placement coordination、activation records、activation storage、activation leases、placement locks、placement command/result types、placement events、placement snapshots、virtual actor activation registry concerns を含む MUST。旧 `identity` と `placement` の public module paths は compatibility shim として残さない MUST。

#### Scenario: identity and placement files move under activation

- **WHEN** implementation 後の cluster-core source tree を確認する
- **THEN** identity lookup と placement coordination の source files は `modules/cluster-core/src/activation/` 配下にある
- **AND** `modules/cluster-core/src/identity.rs` と `modules/cluster-core/src/placement.rs` は compatibility modules として使われていない

#### Scenario: activation owns virtual actor registry

- **WHEN** virtual actor activation ownership と activation cache invalidation types を確認する
- **THEN** `VirtualActorRegistry` と `VirtualActorEvent` は `activation` boundary から公開される
- **AND** caller-facing Grain API files は activation boundary の外側に残る

### Requirement: Grain boundary remains caller-facing

`fraktor-cluster-core-rs` は `grain` module を caller-facing Grain API と message contracts に集中させる SHALL。`grain` boundary は Grain key/reference/context types、call options、retry policy、codecs、serialized message types、RPC routing、schema negotiation、Grain metrics を含む MUST。placement coordination、PID cache、rendezvous hashing、activation registry ownership を吸収しない MUST。

#### Scenario: RPC and codec contracts stay in grain

- **WHEN** Grain RPC、serialization、schema negotiation files を確認する
- **THEN** それらは `modules/cluster-core/src/grain/` 配下に残る
- **AND** activation data が必要な場合のみ `activation` boundary 経由で activation types を import する

### Requirement: Extension boundary owns ActorSystem integration

`fraktor-cluster-core-rs` は ActorSystem integration と cluster entrypoints の module boundary として `extension` を定義する SHALL。この boundary は `ClusterApi`、`ClusterCore`、`ClusterExtension`、extension configuration、extension id、extension installer、provider shared handle、startup mode、extension flows で使う request/resolve/API/cluster/provider errors、cluster entrypoints で使う metrics errors を含む MUST。

#### Scenario: root extension files move under extension

- **WHEN** root-level cluster API、core、extension、installer、config、startup、extension-flow error files を確認する
- **THEN** それらは `modules/cluster-core/src/extension/` 配下で宣言されている
- **AND** `lib.rs` はそれらを flat root sibling modules として宣言していない

### Requirement: Topology boundary owns observed cluster state contracts

`fraktor-cluster-core-rs` は observed cluster state と topology-change contracts の module boundary として `topology` を定義する SHALL。この boundary は cluster events、event types、cluster topology snapshots、topology updates、topology apply errors、cluster metrics snapshots、block-list provider、config validation、join compatibility checking を含む MUST。

#### Scenario: topology contracts move under topology

- **WHEN** cluster event、topology、metrics snapshot、config validation、block-list、join compatibility files を確認する
- **THEN** それらは `modules/cluster-core/src/topology/` 配下で宣言されている
- **AND** extension code は `topology` boundary 経由でこれらの contracts を利用する

### Requirement: Independent cluster boundaries remain parallel

`fraktor-cluster-core-rs` は `membership`、`failure_detector`、`downing_provider`、`pub_sub`、`outbound`、`cluster_provider` を top-level peer boundaries として維持する SHALL。この再編成は `membership` と failure detection / downing policy を統合しない MUST。`pub_sub` と outbound delivery pipeline types を統合しない MUST。

#### Scenario: membership, failure detection, and downing remain separate

- **WHEN** membership、failure detector、downing provider modules を確認する
- **THEN** それらは別々の top-level module boundaries として残る
- **AND** それらをまとめるためだけの新しい umbrella module は導入されていない

#### Scenario: pub-sub and outbound remain separate

- **WHEN** publish/subscribe と outbound pipeline modules を確認する
- **THEN** `pub_sub` と `outbound` は別々の top-level module boundaries として残る
- **AND** それらをまとめるためだけの generic messaging module は導入されていない

### Requirement: Module names are specific and non-ambiguous

cluster-core module reorganization は owned responsibility を識別できる名前を使う SHALL。この package structure のために `runtime`、`manager`、`service`、`engine`、`util`、`facade` という新しい source module names を導入しない MUST。

#### Scenario: forbidden ambiguous module names are absent

- **WHEN** 新しい cluster-core module declarations と directory names を確認する
- **THEN** 新しい organization modules は `runtime`、`manager`、`service`、`engine`、`util`、`facade` を使っていない
- **AND** top-level organization names は `activation`、`extension`、`topology` のように具体的な boundary names である

### Requirement: Old module path compatibility is not preserved

この再編成は新しい境界に合わない旧 public submodule paths を削除する SHALL。pre-change の `identity`、`placement`、flat root-level module paths を守るためだけの compatibility shim modules、deprecated aliases、forwarding modules は追加しない MUST。

#### Scenario: no compatibility shims for moved modules

- **WHEN** implementation 後の module declarations を確認する
- **THEN** 移動した modules は新しい boundaries でのみ宣言されている
- **AND** 旧 module names は compatibility のための forwarding wrappers として残っていない
