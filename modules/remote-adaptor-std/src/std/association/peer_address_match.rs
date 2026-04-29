#[cfg(test)]
mod tests;

use fraktor_remote_core_rs::core::address::Address;

pub(crate) fn peer_matches_address(peer: &str, address: &Address) -> bool {
  let Some((host, port)) = peer.rsplit_once(':') else {
    return false;
  };
  let Ok(port) = port.parse::<u16>() else {
    return false;
  };
  // `SocketAddr` の Display は IPv6 を `[::1]:2552` 形式で書き出すが、`Address.host()` は
  // 括弧なしの `::1` を返す。IPv6 peer を mismatch 扱いにしないため括弧を剥がして比較する。
  let host = host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(host);
  host == address.host() && port == address.port()
}
