//! Remote authority parsing helpers.

#[cfg(test)]
#[path = "remote_authority_test.rs"]
mod tests;

use fraktor_remote_core_rs::address::Address;

pub(crate) fn parse_remote_authority(raw: &str) -> Option<Address> {
  let (system, endpoint) = raw.split_once('@')?;
  if system.is_empty() {
    return None;
  }
  let (host, port) = endpoint.rsplit_once(':')?;
  let host = match (host.starts_with('['), host.ends_with(']')) {
    | (true, true) => &host[1..host.len() - 1],
    | (true, false) | (false, true) => return None,
    | (false, false) => host,
  };
  if host.is_empty() {
    return None;
  }
  let port = port.parse::<u16>().ok()?;
  Some(Address::new(system, host, port))
}
