# actor-core root module flattening

## Goal

Remove the redundant `core::kernel` module layer from `actor-core` and make the file layout match the public module path directly.

## Constraints

- Do not use `#[path]`.
- Do not use `include!`.
- Do not add compatibility re-exports or old-path shims.
- Breaking import changes are allowed because this project has not been released yet.

## Steps

1. Move `modules/actor-core/src/core/kernel/*` to `modules/actor-core/src/*`.
2. Replace `pub mod core` with direct root module declarations in `modules/actor-core/src/lib.rs`.
3. Delete the old `core.rs` and `core/kernel.rs` module wrapper files.
4. Update workspace imports from `fraktor_actor_core_rs::...` to `fraktor_actor_core_rs::...`.
5. Update internal imports from `crate::...` to `crate::...`.
6. Search for remaining `core::kernel`, `#[path]`, `include!`, and compatibility shim leftovers.
7. Run formatting and targeted build/test checks.
