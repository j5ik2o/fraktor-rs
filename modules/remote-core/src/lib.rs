#![deny(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_types, clippy::redundant_clone))]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::missing_safety_doc)]
#![cfg_attr(not(test), deny(clippy::redundant_clone))]
#![deny(clippy::redundant_field_names)]
#![deny(clippy::redundant_pattern)]
#![deny(clippy::redundant_static_lifetimes)]
#![deny(clippy::unnecessary_to_owned)]
#![deny(clippy::unnecessary_struct_initialization)]
#![deny(clippy::needless_borrow)]
#![deny(clippy::needless_pass_by_value)]
#![deny(clippy::manual_ok_or)]
#![deny(clippy::manual_map)]
#![deny(clippy::manual_let_else)]
#![deny(clippy::manual_strip)]
#![deny(clippy::unused_async)]
#![deny(clippy::unused_self)]
#![deny(clippy::unnecessary_wraps)]
#![deny(clippy::unreachable)]
#![deny(clippy::empty_enums)]
#![deny(clippy::no_effect)]
#![deny(dropping_copy_types)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), deny(clippy::expect_used))]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::missing_const_for_fn)]
#![deny(clippy::must_use_candidate)]
#![deny(clippy::trivially_copy_pass_by_ref)]
#![deny(clippy::clone_on_copy)]
#![deny(clippy::len_without_is_empty)]
#![deny(clippy::wrong_self_convention)]
#![deny(clippy::from_over_into)]
#![deny(clippy::eq_op)]
#![deny(clippy::bool_comparison)]
#![deny(clippy::needless_bool)]
#![deny(clippy::match_like_matches_macro)]
#![deny(clippy::manual_assert)]
#![deny(clippy::naive_bytecount)]
#![deny(clippy::if_same_then_else)]
#![deny(clippy::cmp_null)]
#![deny(unreachable_pub)]
#![allow(unknown_lints)]
#![deny(cfg_std_forbid)]
#![cfg_attr(not(test), no_std)]

//! Pekko Artery compatible remote core for the `fraktor` runtime.
//!
//! This crate provides the pure, `no_std`-friendly core logic of the remote subsystem,
//! modelled after Apache Pekko Artery's responsibility split. It contains only data
//! types, state machines, and port (trait) definitions — all I/O, `tokio` task
//! orchestration, and actor lifecycle wiring live in `fraktor-remote-adaptor-std-rs`.
//!
//! ## Pekko Artery correspondence
//!
//! Each submodule mirrors a concrete Pekko Artery component. The table below maps
//! the core modules exposed by this crate to their Pekko counterparts; see
//! `openspec/changes/remote-redesign/design.md` for the full decomposition.
//!
//! | `fraktor-remote-core-rs` module | Pekko Artery counterpart |
//! |---|---|
//! | [`address`]          | `akka.actor.Address` / `UniqueAddress` |
//! | [`association`]      | `akka.remote.artery.Association` (state machine + send queue) |
//! | [`envelope`]         | `akka.remote.artery.OutboundEnvelope` / `InboundEnvelope` |
//! | [`extension`]        | `akka.remote.RemoteActorRefProvider` lifecycle portion |
//! | [`failure_detector`] | `akka.remote.PhiAccrualFailureDetector` |
//! | [`instrument`]       | `akka.remote.artery.RemoteInstrument` + `FlightRecorder` |
//! | [`provider`]         | `akka.remote.RemoteActorRefProvider` (remote path portion) |
//! | [`config`]           | `akka.remote.RemoteSettings` |
//! | [`transport`]        | `akka.remote.artery.RemoteTransport` |
//! | [`watcher`]          | `akka.remote.RemoteWatcher` (state portion only) |
//! | [`wire`]             | `akka.remote.artery.Codecs` (independent binary format) |
//!
//! ## Design invariants
//!
//! - **`no_std` + `alloc`**: `#![cfg_attr(not(test), no_std)]`. Production code must not `use
//!   std::` directly. `alloc` is available.
//! - **No `async`**: The core does not depend on `tokio`, `async-std`, `futures`, or `async-trait`.
//!   All APIs are synchronous.
//! - **`&mut self` principle**: State mutation uses `&mut self`. Internal mutability (e.g.
//!   `SpinSyncMutex` + `&self`) is forbidden in this crate and pushed to adapters.
//! - **Time as argument (monotonic millis)**: `Instant::now()` is never called in this crate. Every
//!   state-transition method accepts `now_ms: u64` — a monotonic millisecond timestamp supplied by
//!   the caller (typically derived from `std::time::Instant` or `tokio::time::Instant` differences
//!   on the adapter side). Wall-clock values are not supported. This keeps every transition a pure
//!   function of `(state, command, now_ms)`.
//! - **Public boundary**: This crate deliberately **does not** re-export its submodule types at the
//!   crate root. Consumers address types through their full submodule path (e.g.
//!   `fraktor_remote_core_rs::association::Association`) so that the responsibility owning a type
//!   is always visible at the call site.
//!
//! See `openspec/changes/remote-redesign/design.md` for the full rationale.

extern crate alloc;

pub mod address;
pub mod association;
pub mod config;
pub mod envelope;
pub mod extension;
pub mod failure_detector;
pub mod instrument;
pub mod provider;
pub mod transport;
pub mod watcher;
pub mod wire;
