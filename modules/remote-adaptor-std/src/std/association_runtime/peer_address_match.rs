use fraktor_remote_core_rs::core::address::Address;

pub(crate) fn peer_matches_address(peer: &str, address: &Address) -> bool {
  let Some((host, port)) = peer.rsplit_once(':') else {
    return false;
  };
  let Ok(port) = port.parse::<u16>() else {
    return false;
  };
  host == address.host() && port == address.port()
}
