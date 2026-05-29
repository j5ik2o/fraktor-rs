## 1. Module Structure

- [x] 1.1 `activation`、`extension`、`topology` の module files/directories を作成する
- [x] 1.2 `lib.rs` の module declarations を新しい top-level boundaries に合わせる
- [x] 1.3 新しい organization module names に `runtime`、`manager`、`service`、`engine`、`util`、`facade` が含まれないことを確認する

## 2. Activation Boundary

- [x] 2.1 `identity` 配下の files を `activation` 配下へ移動し、module declarations と re-exports を更新する
- [x] 2.2 `placement` 配下の files を `activation` 配下へ移動し、module declarations と re-exports を更新する
- [x] 2.3 `VirtualActorRegistry` と `VirtualActorEvent` を `grain` から `activation` へ移動する
- [x] 2.4 旧 `identity` / `placement` compatibility shim modules が残っていないことを確認する

## 3. Grain Boundary

- [x] 3.1 `grain` に残す caller-facing API / RPC / codec / schema / metrics files を確認する
- [x] 3.2 `grain` から activation types を参照する imports を `activation` boundary 経由へ更新する
- [x] 3.3 `grain` が placement coordination、PID cache、rendezvous hashing、activation registry ownership を直接所有していないことを確認する

## 4. Extension and Topology Boundaries

- [x] 4.1 `ClusterApi`、`ClusterCore`、`ClusterExtension`、extension config/id/installer、startup、extension-flow errors を `extension` 配下へ移動する
- [x] 4.2 `BlockListProvider`、cluster events、topology、metrics snapshot、config validation、join compatibility、topology update/apply error を `topology` 配下へ移動する
- [x] 4.3 extension code が topology contracts を `topology` boundary 経由で参照するよう imports を更新する
- [x] 4.4 `lib.rs` の root re-exports を監査し、canonical public entrypoints として意図するものだけ残す

## 5. Preserve Parallel Boundaries

- [x] 5.1 `membership`、`failure_detector`、`downing_provider` が top-level peer boundaries のまま残っていることを確認する
- [x] 5.2 `pub_sub` と `outbound` が別々の top-level peer boundaries のまま残っていることを確認する
- [x] 5.3 `cluster_provider` が provider port と local/static/no-op implementations の境界として残っていることを確認する

## 6. Verification

- [x] 6.1 cluster-core sibling tests と intra-crate imports を新しい module paths に合わせて更新する
- [x] 6.2 `rg` で旧 `crate::identity` / `crate::placement` / flat root module imports が残っていないことを確認する
- [x] 6.3 `MISE_TRUSTED_CONFIG_PATHS=$PWD/mise.toml mise exec -- openspec validate reorganize-cluster-core-modules --strict` を通す
- [x] 6.4 targeted `fraktor-cluster-core-rs` tests を通す
- [x] 6.5 `cargo fmt --check --all` と `git diff --check` を通す
