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

fraktor-rs is a specification-driven actor runtime that brings Pekko- and Proto.Actor-inspired semantics to both `no_std` targets and host runtimes.

It is designed to let you work with a consistent actor model across embedded-friendly environments and standard host environments, while sharing the same overall architecture and workspace structure.

## Highlights

- Shared `core`/`std` module structure across the workspace, so the same actor model can be used on embedded targets and on Tokio-based hosts.
- Six focused crates for `utils`, `actor`, `persistence`, `remote`, `cluster`, and `stream`, plus runnable showcases for common scenarios.
- Pekko / Proto.Actor-inspired semantics for lifecycle, supervision, death watch, actor paths, remoting, and typed/untyped bridging.
- A specification-driven workflow supported by steering, custom dylint rules, and CI scripts for consistent change management.

## Quickstart

### Requirements

- `rustup`
- Rust toolchain `nightly-2025-12-01`
- `cargo-dylint`, `rustc-dev`, and `llvm-tools-preview` if you want to run the full local checks

### Install

```bash
rustup toolchain install nightly-2025-12-01 --component rustfmt --component clippy
git clone git@github.com:j5ik2o/fraktor-rs.git
cd fraktor-rs
```

### Run

```bash
cargo run -p fraktor-showcases-std --example getting_started
```

### Verify

```bash
cargo test -p fraktor-actor-core-kernel-rs --features "std test-support tokio-executor"
./scripts/ci-check.sh all
```

## Workspace layout

The workspace is organized around the following crates:

| Crate | Purpose |
| --- | --- |
| [`modules/utils`](modules/utils) | Portable primitives, runtime helper utilities, atomics, synchronization, and timers |
| [`modules/actor`](modules/actor) | ActorSystem, mailboxes, supervision, typed APIs, scheduler, and EventStream |
| [`modules/persistence`](modules/persistence) | Event sourcing, journals, snapshot stores, and persistent actor support |
| [`modules/remote`](modules/remote) | Remoting extensions, endpoint management, transport adapters, and failure detection |
| [`modules/cluster`](modules/cluster) | Membership, identity lookup, placement, topology, pub-sub, and ECS integration |
| [`modules/stream`](modules/stream) | Reactive stream primitives built on top of the actor system |
| [`modules/actor/examples`](modules/actor/examples) | Focused actor examples such as typed event streams, classic timers, and classic logging |
| [`showcases/std`](showcases/std) | Runnable integrated examples including getting started, request/reply, timers, routing, persistence, remoting, and clustering |

Common entrypoints:

```bash
# Standard showcase
cargo run -p fraktor-showcases-std --example request_reply

# Advanced showcase
cargo run -p fraktor-showcases-std --example remote_messaging --features advanced
```

## Documentation

- API docs: [docs.rs/fraktor-rs](https://docs.rs/fraktor-rs)
- Repository knowledge base: [DeepWiki](https://deepwiki.com/j5ik2o/fraktor-rs)
- Current parity reports:
  - [Actor](docs/gap-analysis/actor-gap-analysis.md)
  - [Remote](docs/gap-analysis/remote-gap-analysis.md)
  - [Cluster](docs/gap-analysis/cluster-gap-analysis.md)
  - [Persistence](docs/gap-analysis/persistence-gap-analysis.md)
  - [Stream](docs/gap-analysis/stream-gap-analysis.md)

## Getting help

- Issues: [GitHub Issues](https://github.com/j5ik2o/fraktor-rs/issues)
- Source of truth for implementation rules: [AGENTS.md](AGENTS.md)

## Contributing

- Follow the repository's specification-driven workflow before implementation.
- Refer to [`.kiro/steering`](.kiro/steering) and [AGENTS.md](AGENTS.md) for project-wide rules.
- Run `./scripts/ci-check.sh all` before opening a PR.
- Clearly describe any impact on runtime, remoting, cluster, or stream behavior in the PR.

## License

Dual-licensed under Apache-2.0 and MIT. See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT).
