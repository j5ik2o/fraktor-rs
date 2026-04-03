# fraktor-rs

[![ci](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/j5ik2o/fraktor-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/fraktor-rs.svg)](https://crates.io/crates/fraktor-rs)
[![docs.rs](https://docs.rs/fraktor-rs/badge.svg)](https://docs.rs/fraktor-rs)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/j5ik2o/fraktor-rs)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

[日本語版](README.ja.md)

fraktor-rs is a specification-driven actor runtime that brings Pekko and Proto.Actor-style semantics to both `no_std` targets and host runtimes. It gives you one workspace with shared `core`/`std` layering for actors, remoting, clustering, and streams instead of maintaining separate embedded and host codebases.

## Highlights

- Shared `core`/`std` module structure across the workspace, so the same actor model can be used on embedded targets and on Tokio-based hosts.
- Five focused crates for utils, actors, remoting, clustering, and streams, plus runnable showcases for the common scenarios.
- Pekko / Proto.Actor inspired semantics for lifecycle, supervision, death watch, actor paths, remoting, and typed/untyped bridging.
- Specification-driven workflow with project steering, custom dylint rules, and reproducible CI entrypoints.

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
cargo test -p fraktor-actor-rs --features "std test-support tokio-executor"
./scripts/ci-check.sh all
```

## Usage

The workspace is organized around these crates:

| Crate | Purpose |
| --- | --- |
| [`modules/utils`](modules/utils) | Portable primitives, runtime toolbox, atomics, synchronization, timers |
| [`modules/actor`](modules/actor) | ActorSystem, mailboxes, supervision, typed APIs, scheduler, EventStream |
| [`modules/remote`](modules/remote) | Remoting extension, endpoint management, transport adapters, failure detection |
| [`modules/cluster`](modules/cluster) | Membership, identity lookup, placement, topology, pub-sub, ECS integration |
| [`modules/stream`](modules/stream) | Reactive stream primitives built on the actor system |
| [`showcases/std`](showcases/std) | Runnable examples such as getting started, request/reply, timers, routing, remoting, and clustering |

Common entrypoints:

```bash
# Run a standard showcase
cargo run -p fraktor-showcases-std --example request_reply

# Run an advanced showcase
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

- Follow the repository's spec-driven workflow before implementation.
- Respect the project-wide rules in [`.kiro/steering`](.kiro/steering) and [AGENTS.md](AGENTS.md).
- Run `./scripts/ci-check.sh all` before opening a PR.
- Use a focused branch and describe runtime, remoting, cluster, or stream impact in the PR.

## License

Dual-licensed under Apache-2.0 and MIT. See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT).
