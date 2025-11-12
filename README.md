# fraktor-rs

[![CI](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml)
[![License: Apache-2.0 / MIT](https://img.shields.io/badge/license-Apache--2.0%20%2F%20MIT-blue.svg)](LICENSE-MIT)

> See [README.ja.md](README.ja.md) for the Japanese edition.

fraktor-rs is a specification-driven actor runtime that mirrors the lifecycle, supervision, and remoting patterns of Akka/Pekko and protoactor-go while remaining friendly to both `no_std` microcontrollers and host targets such as Tokio. The workspace ships a consistent API surface across `actor-core` (portable runtime), `actor-std` (host adapters), and `utils-*` crates, making it possible to deploy the same behaviors from RP2040 boards to Linux servers.

## Key Capabilities
- **Lifecycle-first ActorSystem** – `SystemMessage::{Create,Recreate,Failure}` takes priority in the system mailbox to guarantee deterministic supervision and DeathWatch flows.
- **Pekko-compatible ActorPath** – `ActorPathParts`, `PathSegment`, and `ActorPathFormatter` build `pekko://system@host:port/user/...` URIs with guardian injection, UID suffixes, and RFC2396 validation.
- **Remote authority management** – `RemoteAuthorityManager` tracks `Unresolved → Connected → Quarantine` transitions, defers outbound traffic, and enforces quarantine timeouts sourced from `RemotingConfig`.
- **Observability tooling** – EventStream, DeadLetter, and LoggerSubscriber expose low-latency telemetry pipelines that span RTT/UART on embedded boards and tracing subscribers on hosts.
- **Typed/Untyped interop** – `TypedActor` bridges into classic actors via `into_untyped`/`as_untyped`, preserving reply semantics without global sender state.
- **Toolbox abstraction** – `fraktor-utils-core` provides `RuntimeToolbox` primitives (portable atomics, spinlocks, timers) so higher layers stay allocator-agnostic and interrupt-safe.

## Architecture Overview
```
utils-core  -->  actor-core  -->  actor-std
   ^              ^                ^
   |              |                |
   |          Remoting &       Tokio/host
   |          system APIs      integration
```
- **utils-core**: low-level synchronization, URI parser, timer families, ArcShared replacements.
- **actor-core**: no_std ActorSystem, actor refs, mailboxes, supervision, actor path registry, remote authority management.
- **actor-std**: Tokio executors, logging backends, host-only conveniences layered on `actor-core`.
- **utils-std**: complements utils-core with std-only helpers when the target allows it.

## Getting Started
1. **Install prerequisites**
   - Rust nightly toolchain (`rustup toolchain install nightly`)
   - `cargo-dylint`, `rustc-dev`, `llvm-tools-preview` (for custom lints)
   - Optional: `rustup target add thumbv6m-none-eabi thumbv8m.main-none-eabi` for embedded builds
2. **Clone the repo**
   ```bash
   git clone git@github.com:j5ik2o/fraktor-rs.git
   cd fraktor-rs
   ```
3. **Run the developer workflow**
   ```bash
   cargo fmt --check
   cargo test -p fraktor-utils-core-rs uri_parser
   cargo test -p fraktor-actor-core-rs actor_path
   scripts/ci-check.sh all   # full lint + dylint + no_std/std/embedded + docs
   ```

## Repository Layout
| Path | Description |
| --- | --- |
| `modules/utils-core` | no_std primitives: portable atomics, RuntimeToolbox, RFC2396 URI parser |
| `modules/actor-core` | platform-neutral ActorSystem, actor path registry, remote authority manager |
| `modules/actor-std` | Tokio bindings, host logging/telemetry bridges |
| `modules/utils-std` | std-only helpers layered on utils-core |
| `scripts/` | repeatable CI entry points (lint, dylint, no_std, std, embedded, docs) |
| `.kiro/` | OpenSpec-driven requirements/design/tasks plus steering policies |

## Specification-Driven Development
Large features follow the OpenSpec flow captured in `.kiro/specs/<feature>`:
1. **Requirements** → **Design** → **Tasks** → **Implementation** (`/prompts:kiro-*` commands)
2. Steering policies under `.kiro/steering/` define coding standards (2018 modules, 1 type per file, rustdoc in English, non-rustdoc in Japanese unless otherwise stated).
3. Validation is automated via `/prompts:kiro-validate-*` to ensure requirements traceability and test coverage before merging.

## Roadmap
- Complete ActorSelection resolver enhancements (`/system` guardians, `..` navigation).
- Finalize `RemoteAuthorityManager` quarantine timers and deferred queues.
- Implement end-to-end Pekko-compatible remoting transport after the registry/authority layers stabilize.
- Continue enriching docs (`docs/guides`) and add bilingual coverage to match README.ja.md.

## Contributing
1. Fork and create a feature branch.
2. Add/extend specifications under `.kiro/specs/` before touching code.
3. Run `scripts/ci-check.sh all` locally; ensure both no_std and std test suites pass.
4. Submit a PR referencing the relevant spec + tasks.

## License
Dual-licensed under Apache-2.0 and MIT. See `LICENSE-APACHE` and `LICENSE-MIT` for details.
