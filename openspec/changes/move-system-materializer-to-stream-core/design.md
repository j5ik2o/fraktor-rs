## Context

`stream-core-kernel` is the no_std stream runtime crate. It owns materialization contracts such as `ActorMaterializer`, `ActorMaterializerConfig`, `Materializer`, and snapshot support.

`stream-adaptor-std` is the std adapter crate. Its current IO surface (`FileIO`, `StreamConverters`, `StreamInputStream`, `StreamOutputStream`) depends on `std::fs`, `std::io`, and std channels. `SystemMaterializer` is different: it wraps an `ActorMaterializer` and implements the actor-core extension contract. Its current std dependency is only `std::vec::Vec`, which can be replaced by `alloc::vec::Vec`.

## Goals / Non-Goals

**Goals:**

- Make `SystemMaterializer` and `SystemMaterializerId` core materialization APIs.
- Keep the crate dependency direction as `stream-adaptor-std -> stream-core-kernel`.
- Align the runtime meaning with DIP: core logic depends on core contracts; std adapter crates provide platform-specific implementations.
- Remove the misleading std materializer public module.
- Preserve no_std compatibility for `stream-core-kernel`.

**Non-Goals:**

- Do not add a new materializer port trait in this change.
- Do not redesign `ActorMaterializer`, actor-system extension storage, scheduler, or tick driver APIs.
- Do not keep a compatibility re-export from `stream-adaptor-std`.
- Do not move `FileIO` or `StreamConverters` out of the std adapter crate.

## Decisions

### Decision 1: Move the types to `stream-core-kernel::materialization`

`SystemMaterializer` and `SystemMaterializerId` will live beside `ActorMaterializer` because they are part of the materialization runtime contract, not a host adapter. This keeps the public path aligned with the concept:

```text
fraktor_stream_core_kernel_rs::materialization::{SystemMaterializer, SystemMaterializerId}
```

Alternative considered: keep a re-export in `stream-adaptor-std`. This is rejected because the project is pre-release and favors clean contracts over compatibility shims.

### Decision 2: Keep platform-dependent pieces outside this move

The existing `SystemMaterializerId::create_extension` constructs `ActorMaterializer` from the already-created `ActorSystem`. It does not own std execution. Platform-specific scheduler/tick behavior remains attached to the actor system configuration and std driver implementations.

Alternative considered: introduce a new materializer port trait now. This is rejected because no separate std-only implementation point is needed for the current type; adding a port would be speculative.

### Decision 3: Use `alloc`, not `std`, in core

`SystemMaterializer::stream_snapshots` returns `Vec<StreamSnapshot>`. In core this must be `alloc::vec::Vec`. The implementation must not import `std::*`, must not add a default feature, and must continue to satisfy `cfg_std_forbid`.

### Decision 4: Tests define both behavior and package boundary

The existing behavior tests move with the type into `stream-core-kernel`. A public API test must confirm the new external import path. The std package-boundary test must stop importing materializer types and continue to assert only std adapter exports.

## Risks / Trade-offs

- **Breaking import paths** → Accept the break and document it in the proposal; no compatibility shim is allowed.
- **Core accidentally gains std imports** → Use `alloc::vec::Vec` and verify with no-default-features check plus existing cfg-std-forbid lint coverage.
- **Spec drift from older stream package wording** → Modify the existing `stream-package-structure` requirement so archive will replace the old std materializer contract.
- **Tests may rely on std-only test helpers** → Keep those dependencies as dev-dependencies of `stream-core-kernel`; production code remains no_std.
