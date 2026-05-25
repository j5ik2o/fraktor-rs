## Why

`SystemMaterializer` is currently exported from `stream-adaptor-std`, but it is not a `std::io`, Tokio, filesystem, or networking adapter. It is a stream core materialization concept: an actor-system extension that owns the shared `ActorMaterializer`.

Keeping it in the std adapter crate blurs the ports-and-adapters boundary. The core logic should depend on core contracts, while std crates provide platform-specific implementations through those contracts.

## What Changes

- **BREAKING**: Move `SystemMaterializer` and `SystemMaterializerId` from `fraktor_stream_adaptor_std_rs::materializer` to `fraktor_stream_core_kernel_rs::materialization`.
- Remove the `stream-adaptor-std` materializer public module instead of leaving a compatibility re-export.
- Keep std-specific stream APIs such as `FileIO`, `StreamConverters`, `StreamInputStream`, and `StreamOutputStream` in `stream-adaptor-std`.
- Replace the current `std::vec::Vec` use in `SystemMaterializer` with `alloc::vec::Vec` so the type remains valid in the no_std core crate.
- Preserve `SystemMaterializerId` as an actor-core `ExtensionId` that constructs `ActorMaterializer::new(system.clone(), ActorMaterializerConfig::new())`.
- Update tests so core owns the materializer behavior and public API contract, while std package-boundary tests only assert std adapter exports.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `stream-package-structure`: System materializer ownership moves from the std adapter package to the stream core materialization package, and the std adapter package is narrowed to actual std-backed adapters.

## Impact

- Public API callers must import `SystemMaterializer` and `SystemMaterializerId` from `fraktor_stream_core_kernel_rs::materialization`.
- `fraktor_stream_adaptor_std_rs::materializer::*` is removed.
- `modules/stream-core-kernel` gains the materializer extension types and tests.
- `modules/stream-adaptor-std` keeps IO adapter types only.
- The no_std boundary must continue to reject `std::*` use in `stream-core-kernel`.
