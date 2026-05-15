//! Remote authority parsing helpers.

#[cfg(test)]
#[path = "remote_authority_test.rs"]
mod tests;

use fraktor_remote_core_rs::address::Address;

pub(crate) fn parse_remote_authority(raw: &str) -> Option<Address> {
  let (system, endpoint) = raw.split_once('@')?;
  let (host, port) = endpoint.rsplit_once(':')?;
  let host = host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(host);
  Some(Address::new(system, host, port.parse::<u16>().ok()?))
}
