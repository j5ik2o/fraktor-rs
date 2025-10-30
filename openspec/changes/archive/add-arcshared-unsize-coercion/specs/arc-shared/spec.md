# Arc Shared

## ADDED Requirements
### Requirement: Nightly ArcShared unsized coercion support
`ArcShared` MUST allow coercing concrete types to trait-object forms when the nightly-only `unsize` feature is enabled.

#### Scenario: Coerce ArcShared to trait object without into_dyn
- **WHEN** the crate is compiled with nightly Rust and the `unsize` feature enabled
- **AND** a concrete type `Concrete` implements a trait `Trait`
- **AND** `let ptr: ArcShared<Concrete> = ArcShared::new(Concrete);` is assigned to `let dyn_ptr: ArcShared<dyn Trait> = ptr;`
- **THEN** the assignment compiles without requiring an explicit conversion method
- **AND** the resulting `ArcShared<dyn Trait>` points to the same underlying allocation provided by the selected Arc backend (`portable_atomic_util::Arc` when `force-portable-arc` is enabled, otherwise `alloc::sync::Arc`).

#### Scenario: Stable builds remain unaffected
- **WHEN** the crate is compiled without the `unsize` feature
- **THEN** a stable Rust build succeeds without enabling nightly feature gates
- **AND** existing `ArcShared` APIs continue to function as before.
