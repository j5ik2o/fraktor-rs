use fraktor_remote_core_rs::config::RemoteConfig;

use super::*;

#[test]
fn from_config_applies_inbound_and_outbound_lane_counts() {
  let config = RemoteConfig::new("127.0.0.1").with_inbound_lanes(3).with_outbound_lanes(4);
  let transport = TcpRemoteTransport::from_config("local-sys", config);

  assert_eq!(transport.inbound_lanes, 3);
  assert_eq!(transport.outbound_lanes, 4);
  assert_eq!(transport.inbound_txs.len(), 3);
  assert_eq!(transport.inbound_rxs.as_ref().expect("inbound receivers").len(), 3);
}
