# fraktor-rs

[![ci](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/fraktor-rs.svg)](https://crates.io/crates/fraktor-rs)
[![docs.rs](https://docs.rs/fraktor-rs/badge.svg)](https://docs.rs/fraktor-rs)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/j5ik2o/fraktor-rs)
[![Renovate](https://img.shields.io/badge/renovate-enabled-brightgreen.svg)](https://renovatebot.com)
[![dependency status](https://deps.rs/repo/github/j5ik2o/fraktor-rs/status.svg)](https://deps.rs/repo/github/j5ik2o/fraktor-rs)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![License](https://img.shields.io/badge/License-APACHE2.0-blue.svg)](https://opensource.org/licenses/apache-2-0)

[日本語版](README.ja.md)

fraktor-rs is a specification-driven Rust actor runtime inspired by Apache Pekko and Proto.Actor.
The runtime is developed as `no_std` core crates plus `std` adaptor crates, keeping portable state machines and contracts separate from Tokio, networking, and host-runtime bindings.

The root `fraktor-rs` crate currently publishes project metadata and reserves the package name.
Runtime APIs live in the workspace crates under [`modules/`](modules), and the fastest way to inspect behavior is through the runnable [`fraktor-showcases-std`](showcases/std) examples.

## Highlights

- Portable `no_std` core crates for actor kernel, typed actors, persistence, remote, cluster, streams, and shared utilities.
- `std` adaptor crates isolate host-specific concerns such as Tokio executors, TCP transport, std locks, materializers, and cluster delivery helpers.
- Pekko / Proto.Actor-inspired semantics for actor systems, supervision, death watch, routing, dispatchers, mailboxes, event streams, serialization, remoting, clustering, persistence, and stream processing.
- Runnable std showcases cover legacy typed flows, Pekko classic/kernel examples, typed examples, stream examples, and advanced remote/persistence scenarios.
- OpenSpec artifacts, repository rules, custom dylint checks, and CI scripts keep design intent, module boundaries, and implementation checks aligned.

## Quickstart

### Requirements

- `rustup`
- Rust toolchain `nightly-2025-12-01` (pinned by [`rust-toolchain.toml`](rust-toolchain.toml))
- `cargo-dylint`, `rustc-dev`, and `llvm-tools-preview` for the full local check suite

### Install

```bash
git clone git@github.com:j5ik2o/fraktor-rs.git
cd fraktor-rs
rustup toolchain install nightly-2025-12-01 --component rustfmt --component clippy
```

For full dylint-backed verification:

```bash
rustup component add rustc-dev llvm-tools-preview --toolchain nightly-2025-12-01
cargo install cargo-dylint dylint-link
```

### Run

```bash
cargo run -p fraktor-showcases-std --example getting_started
```

More examples:

```bash
cargo run -p fraktor-showcases-std --example typed_first_example
cargo run -p fraktor-showcases-std --example stream_first_example
cargo run -p fraktor-showcases-std --features advanced --example remote_lifecycle
```

### Verify

```bash
cargo test -p fraktor-rs
./scripts/ci-check.sh ai all
```

## Usage

Until the root facade exposes consolidated runtime modules, use the crate that owns the API area you need:

| Area | Crates |
| --- | --- |
| Utilities | [`fraktor-utils-core-rs`](modules/utils-core), [`fraktor-utils-adaptor-std-rs`](modules/utils-adaptor-std) |
| Actor runtime | [`fraktor-actor-core-kernel-rs`](modules/actor-core-kernel), [`fraktor-actor-core-typed-rs`](modules/actor-core-typed), [`fraktor-actor-adaptor-std-rs`](modules/actor-adaptor-std) |
| Persistence | [`fraktor-persistence-core-kernel-rs`](modules/persistence-core-kernel), [`fraktor-persistence-core-typed-rs`](modules/persistence-core-typed) |
| Remote | [`fraktor-remote-core-rs`](modules/remote-core), [`fraktor-remote-adaptor-std-rs`](modules/remote-adaptor-std) |
| Cluster | [`fraktor-cluster-core-rs`](modules/cluster-core), [`fraktor-cluster-adaptor-std-rs`](modules/cluster-adaptor-std) |
| Streams | [`fraktor-stream-core-kernel-rs`](modules/stream-core-kernel), [`fraktor-stream-core-actor-typed-rs`](modules/stream-core-actor-typed), [`fraktor-stream-adaptor-std-rs`](modules/stream-adaptor-std) |

The showcase crate is the current usage index for executable flows:

```bash
cargo run -p fraktor-showcases-std --example request_reply
cargo run -p fraktor-showcases-std --example kernel_supervision
cargo run -p fraktor-showcases-std --example typed_actor_lifecycle
cargo run -p fraktor-showcases-std --example stream_graphs
cargo run -p fraktor-showcases-std --features advanced --example typed_persistence_effector
```

See [`showcases/std/README.md`](showcases/std/README.md) for the full example list and feature requirements.

## Workspace Layout

| Path | Purpose |
| --- | --- |
| [`src/`](src) | Root `fraktor-rs` crate placeholder and package metadata |
| [`modules/utils-core`](modules/utils-core) | Portable collections, sync primitives, time helpers, atomics, and network parsing |
| [`modules/utils-adaptor-std`](modules/utils-adaptor-std) | Standard-library utility adapters |
| [`modules/actor-core-kernel`](modules/actor-core-kernel) | `no_std` untyped actor kernel: actor refs, systems, dispatch, routing, serialization, patterns, and lifecycle |
| [`modules/actor-core-typed`](modules/actor-core-typed) | `no_std` typed actor facade, DSL, receptionist, pub-sub, delivery, typed event stream, and typed system APIs |
| [`modules/actor-adaptor-std`](modules/actor-adaptor-std) | Std/Tokio actor bindings, executors, tick drivers, time, event, pattern, and test-support helpers |
| [`modules/persistence-core-kernel`](modules/persistence-core-kernel) | Event sourcing, journals, snapshots, persistent actors, persistent FSM, durable state, and persistence extensions |
| [`modules/persistence-core-typed`](modules/persistence-core-typed) | Persistence effector API, snapshot criteria, and retention criteria for typed actors |
| [`modules/remote-core`](modules/remote-core) | `no_std` remote address, association, envelope, provider, transport port, watcher, wire, and failure-detector state machines |
| [`modules/remote-adaptor-std`](modules/remote-adaptor-std) | Std remote extension installers, providers, Tokio TCP transport, and I/O workers |
| [`modules/cluster-core`](modules/cluster-core) | Cluster membership, identity, placement, pub-sub, grains, failure detection, topology, metrics, and routing |
| [`modules/cluster-adaptor-std`](modules/cluster-adaptor-std) | Std cluster API, local provider wrapping, Tokio gossip transport, pub-sub delivery, and optional AWS ECS provider |
| [`modules/stream-core-kernel`](modules/stream-core-kernel) | `no_std` stream DSL, stages, materialization contracts, graph shapes, stream refs, queues, kill switches, and supervision |
| [`modules/stream-core-actor-typed`](modules/stream-core-actor-typed) | Typed actor integrations for stream DSLs |
| [`modules/stream-adaptor-std`](modules/stream-adaptor-std) | Std stream I/O and materializer adapters |
| [`showcases/std`](showcases/std) | Runnable examples for host environments |
| [`tests/e2e`](tests/e2e) | Cross-crate end-to-end tests |
| [`lints/`](lints) | Custom dylint rules for project structure and Rust conventions |
| [`openspec/`](openspec) | Specification-driven design artifacts, active changes, and accepted specs |

## Documentation

- API docs: [docs.rs/fraktor-rs](https://docs.rs/fraktor-rs)
- Showcase index: [`showcases/std/README.md`](showcases/std/README.md)
- Repository rules: [AGENTS.md](AGENTS.md), [`.agents/rules/project.md`](.agents/rules/project.md)
- OpenSpec configuration: [`openspec/config.yaml`](openspec/config.yaml)
- Lock-free design notes: [`docs/guides/lock_free_design.md`](docs/guides/lock_free_design.md)
- Current gap reports:
  - [Actor](docs/gap-analysis/actor-gap-analysis.md)
  - [Actor mailbox](docs/gap-analysis/actor-mailbox-gap-analysis.md)
  - [Remote](docs/gap-analysis/remote-gap-analysis.md)
  - [Cluster](docs/gap-analysis/cluster-gap-analysis.md)
  - [Persistence](docs/gap-analysis/persistence-gap-analysis.md)
  - [Stream](docs/gap-analysis/stream-gap-analysis.md)
- Reference implementations:
  - [`references/pekko`](references/pekko)
  - [`references/protoactor-go`](references/protoactor-go)

## Getting Help

- Issues: [GitHub Issues](https://github.com/j5ik2o/fraktor-rs/issues)
- Repository knowledge base: [DeepWiki](https://deepwiki.com/j5ik2o/fraktor-rs)

## Contributing

- Read [AGENTS.md](AGENTS.md) and the scoped rules under [`.agents/rules/`](.agents/rules) before changing code.
- Use OpenSpec for behavior-affecting changes; run OpenSpec commands through `mise exec -- openspec ...`.
- Keep `*-core` crates `no_std`; place host-specific runtime, network, time, and Tokio work in `*-adaptor-std` crates.
- Put executable examples in [`showcases/std`](showcases/std), not under `modules/**/examples`.
- Run targeted checks while developing and `./scripts/ci-check.sh ai all` before opening a PR.
- Do not edit [`CHANGELOG.md`](CHANGELOG.md) manually; it is generated by GitHub Actions.

## License

Dual-licensed under Apache-2.0 and MIT. See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT).
