# ai_fix 計画: classic routing 公開面の是正

## 対象 finding

- `routing-001`
- `routing-002`
- `routing-003`

## 方針

1. Pekko parity が未完成な classic routing API は crate 外へ公開しない。
2. 既存の内部実装と単体テストは維持し、公開面だけを最小変更で是正する。
3. 公開 API fixture と compile-fail fixture を更新し、誤った公開が再発しないようにする。

## 変更対象

- `modules/actor-core/src/core/kernel/routing.rs`
- `modules/actor-core/src/core/kernel/routing/router_config.rs`
- `modules/actor-core/src/core/kernel/routing/custom_router_config.rs`
- `modules/actor-core/src/core/kernel/routing/group.rs`
- `modules/actor-core/src/core/kernel/routing/pool.rs`
- `modules/actor-core/src/core/kernel/routing/consistent_hashing_routing_logic.rs`
- `modules/actor-core/src/core/kernel/routing/smallest_mailbox_routing_logic.rs`
- `modules/actor-core/tests/fixtures/kernel_public_surface/public_api.rs`
- `modules/actor-core/tests/fixtures/kernel_public_surface/internal_helpers.rs`
- `modules/actor-core/tests/router_config.rs`

## 検証

- `cargo test -p fraktor-actor-core-rs --test kernel_public_surface`
- `cargo test -p fraktor-actor-core-rs routing::`
- `./scripts/ci-check.sh ai dylint -m actor-core`
