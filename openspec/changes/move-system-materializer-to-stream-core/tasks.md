## 1. Core Materialization Surface

- [ ] 1.1 Add `SystemMaterializer` under `modules/stream-core-kernel/src/materialization/`, replacing `std::vec::Vec` with `alloc::vec::Vec`.
- [ ] 1.2 Add `SystemMaterializerId` under `modules/stream-core-kernel/src/materialization/`, preserving the actor-core `ExtensionId` contract.
- [ ] 1.3 Export both types from `stream-core-kernel::materialization`.
- [ ] 1.4 Update rustdoc links to point at the core materialization path.

## 2. Std Adapter Boundary

- [ ] 2.1 Remove the `stream-adaptor-std` materializer module and the old `SystemMaterializer` / `SystemMaterializerId` definitions.
- [ ] 2.2 Keep `stream-adaptor-std` IO adapters (`FileIO`, `StreamConverters`, `StreamInputStream`, `StreamOutputStream`, `SourceFactory`) unchanged except for import fallout.
- [ ] 2.3 Do not add compatibility re-exports from `stream-adaptor-std`.

## 3. Tests

- [ ] 3.1 Move the existing `SystemMaterializer` behavior tests into `stream-core-kernel` as sibling `*_test.rs` tests.
- [ ] 3.2 Add or update a core public API test for `fraktor_stream_core_kernel_rs::materialization::{SystemMaterializer, SystemMaterializerId}`.
- [ ] 3.3 Update `stream-adaptor-std` package-boundary tests so they assert only std adapter exports.

## 4. Verification

- [ ] 4.1 Run `cargo test -p fraktor-stream-core-kernel-rs system_materializer`.
- [ ] 4.2 Run `cargo test -p fraktor-stream-adaptor-std-rs package_boundaries`.
- [ ] 4.3 Run `cargo check -p fraktor-stream-core-kernel-rs --no-default-features`.
- [ ] 4.4 Run `cargo fmt --check --all`.
