# async-first-actor-adapters implementation plan

## Scope

- Keep actor receive / invoker / mailbox contracts synchronous.
- Add a Tokio task executor for default actor dispatch while preserving the existing `TokioExecutor` as the `spawn_blocking` executor.
- Add an opt-in std Tokio actor-system config helper.
- Keep future-to-message behavior on the existing `pipe_to_self` / `pipe_to` contract.
- Add the Embassy adapter crate and its tick-driver kind without introducing Embassy dependencies into `actor-core-kernel`.
- Add a std showcase under `showcases/std` for this feature.

## Steps

1. Confirm current OpenSpec tasks, existing dispatcher APIs, and std adapter patterns.
2. Implement `TokioTaskExecutor` / `TokioTaskExecutorFactory`, update public re-exports, and add focused tests.
3. Add std Tokio actor-system helper wiring default dispatch to `TokioTaskExecutorFactory`, blocking dispatch to `TokioExecutorFactory`, and ticks to `TokioTickDriver`.
4. Lock the existing untyped and typed future-to-message contracts with tests and docs where gaps exist.
5. Add the Embassy adapter crate, Embassy tick driver, mailbox clock helper, and compile checks.
6. Add `showcases/std/typed/async-first-actor-adapters/main.rs` and register `typed_async_first_actor_adapters`.
7. Run targeted checks first, then the final `./scripts/ci-check.sh ai all`.
