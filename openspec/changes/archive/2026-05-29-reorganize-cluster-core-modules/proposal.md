## Why

`modules/cluster-core/src` は現在、extension wiring、topology state、Grain API、identity lookup、placement activation が crate root または平坦な sibling module に混在している。直近の cluster 変更では identity lookup、placement coordination、virtual actor activation が繰り返し同時に変更されており、現在の配置は実際の変更境界を表現できていない。

## What Changes

- **BREAKING**: `fraktor-cluster-core-rs` の public module path を再編成し、互換 shim や deprecated alias は残さない。
- 分散 Grain activation の境界として `activation` module を導入する。対象は identity lookup、PID cache、rendezvous hashing、placement coordination、activation record、lease、virtual actor activation registry。
- `grain` は Grain-facing API、reference、context、RPC routing、serialization、codec、schema negotiation、metrics に集中させる。
- root-level cluster files を `extension` と `topology` に分割する。
  - `extension`: ActorSystem integration、startup/shutdown API、extension installer/configuration、request/resolve errors、provider shared handle、startup mode、metrics error。
  - `topology`: cluster events、event types、topology snapshots/updates、topology apply errors、metrics snapshots、block-list provider、config validation、join compatibility。
- `membership`、`failure_detector`、`downing_provider` は並列境界として維持する。
- `pub_sub` と `outbound` は別境界として維持する。
- 新しい package structure では `runtime`、`manager`、`service`、`engine` などの曖昧な module 名を導入しない。

## Capabilities

### New Capabilities

- `cluster-core-module-organization`: `fraktor-cluster-core-rs` の source module organization と public module path contract を定義する。

### Modified Capabilities

- None.

## Impact

- `modules/cluster-core/src/lib.rs`
- `modules/cluster-core/src/{activation,extension,topology,grain}/`
- 既存の `modules/cluster-core/src/identity/` と `modules/cluster-core/src/placement/` module path は削除または `activation` 配下へ移動する。
- 既存の root-level `cluster_*`、topology、metrics、config、error files は `extension` または `topology` 配下へ移動する。
- cluster-core tests と intra-crate imports は新しい module path へ更新する。
- 未リリースの submodule path を使う外部利用者は import 更新が必要になる。root re-export は canonical public surface として意図するものだけ維持できる。
