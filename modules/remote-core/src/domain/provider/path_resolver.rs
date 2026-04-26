//! Stateless helper that resolves an [`ActorPath`] into a [`UniqueAddress`].

use alloc::string::ToString;

use fraktor_actor_core_rs::core::kernel::actor::actor_path::ActorPath;

use crate::domain::address::{Address, UniqueAddress};

/// Resolves the remote [`UniqueAddress`] embedded in an [`ActorPath`].
///
/// Returns `None` when the path has no authority component, which is how
/// local (authority-less) actor paths surface through this function. The
/// result has a `uid` of `0` when the path does not carry one — callers
/// should treat `uid == 0` as "unconfirmed" per design Decision 13.
///
/// This is a **free function** on purpose; the helper has no state and does
/// not belong on a struct. Adapters that need to dispatch a path can call
/// this to extract the target address without instantiating a provider.
#[must_use]
pub fn resolve_remote_address(path: &ActorPath) -> Option<UniqueAddress> {
  let parts = path.parts();
  let endpoint = parts.authority_endpoint()?;
  // `authority_endpoint()` is either `"host"` or `"host:port"`.
  let (host, port) = match endpoint.rfind(':') {
    | Some(idx) => {
      let host_part = &endpoint[..idx];
      let port_part = &endpoint[idx + 1..];
      match port_part.parse::<u16>() {
        | Ok(p) => (host_part.to_string(), p),
        | Err(_) => (endpoint.clone(), 0),
      }
    },
    | None => (endpoint, 0),
  };
  let address = Address::new(parts.system().to_string(), host, port);
  let uid = path.uid().map_or(0, |u| u.value());
  Some(UniqueAddress::new(address, uid))
}
