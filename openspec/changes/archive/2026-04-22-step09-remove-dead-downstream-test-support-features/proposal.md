## Why

step06 で `actor-core/test-support` feature を退役した際、下流クレートの `test-support` features は **「独自責務がある可能性」を理由にスコープ外** とした (step06 Non-Goals)。

step06 archive 後の調査で、実際には以下が **dead code** だと確定した:

| crate | test-support 定義 | src 内の `feature = "test-support"` ゲート |
|-------|------------------|--------------------------------------|
| `cluster-core` | `test-support = []` | **0 件** |
| `cluster-adaptor-std` | `test-support = ["fraktor-cluster-core-rs/test-support"]` | **0 件** (forward だけで自分は使っていない) |
| `remote-adaptor-std` | `test-support = []` | **0 件** |

これらは **何の挙動も変えない空 feature**。残しておくと:
- 「何のために存在するの?」誤解を生む
- step06 で消し損ねた残骸という見え方になる
- 下流の `features = ["test-support"]` 指定が意味なく lingering する (showcases/std + cluster-adaptor-std の dev-dep)

`actor-adaptor-std/test-support` は **保持** する。`TestTickDriver` / `new_empty_actor_system*` の公開ゲート (3 件: `std.rs:11` `pub mod system;`、`tick_driver.rs:4` `mod test_tick_driver;`、`tick_driver.rs:13` `pub use ::TestTickDriver;`) と、test 専用ゲート (1 件: `circuit_breakers_registry_id.rs:7` `mod tests;`) の計 4 箇所で実用されている (step03 で確立した役割)。

本 change で 3 crate の dead test-support feature を退役し、Strategy B 由来の test-support クリーンアップを完全に閉じる。

## What Changes

- **削除する feature 定義 (3 件):**
  - `modules/cluster-core/Cargo.toml:17` `test-support = []`
  - `modules/cluster-adaptor-std/Cargo.toml:18` `test-support = ["fraktor-cluster-core-rs/test-support"]`
  - `modules/remote-adaptor-std/Cargo.toml:17` `test-support = []`

- **削除する `features = ["test-support"]` 指定 (3 件):**
  - `showcases/std/Cargo.toml:20` `fraktor-cluster-core-rs = { ..., features = ["test-support"], optional = true }` から削除
  - `showcases/std/Cargo.toml:21` `fraktor-cluster-adaptor-std-rs = { ..., features = ["test-support"], optional = true }` から削除
  - `modules/cluster-adaptor-std/Cargo.toml:37` dev-dep `fraktor-cluster-core-rs = { workspace = true, features = ["test-support"] }` から削除 (= 行ごと削除、prod dep と同等になるため)

- **保持するもの:**
  - `actor-adaptor-std/test-support`: 実用ゲートあり (`tick_driver.rs:4,13`、`std.rs:11`、`circuit_breakers_registry_id.rs:7`)
  - `actor-adaptor-std/Cargo.toml:49,54` の `[[test]] required-features = ["test-support"]`: 上記実用 feature と紐づくため残す

- **BREAKING (workspace-internal、ほぼ影響なし):** 存在しない feature の指定が `Cargo.toml` に残っていても pre-release phase では検出されやすい

**Non-Goals:**
- `actor-adaptor-std/test-support` の見直し (実用ゲートを持つため別 change)
- `cluster-core` 等の他 feature 見直し (本 change は test-support のみ)
- 他の dead feature 一掃 (本 change は test-support 関連のみ)

## Capabilities

### New Capabilities
- なし

### Modified Capabilities
- `actor-test-driver-placement`: step06 で追加した Requirement「actor-core では feature ゲート経由で内部 API の可視性を拡大してはならない」を補強する形で、**「下流 crate の `test-support` feature は実用ゲートを持つ場合のみ存在してよい」** Scenario を追加。空 feature や forward 専用 feature の存在を spec で禁止する

## Impact

- **Affected code**:
  - `modules/cluster-core/Cargo.toml`、`modules/cluster-adaptor-std/Cargo.toml`、`modules/remote-adaptor-std/Cargo.toml` (feature 削除)
  - `showcases/std/Cargo.toml`、`modules/cluster-adaptor-std/Cargo.toml` (dev-dep / dep の features 指定削除)
- **Affected APIs**: なし (空 feature の削除であり挙動変化なし)
- **Affected dependencies**: なし
- **Release impact**: pre-release phase につき外部影響軽微
