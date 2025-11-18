//! Tests for TokioTcpTransport.

use core::time::Duration;
use std::thread;

use fraktor_actor_rs::core::event_stream::CorrelationId;
use tokio::runtime::Runtime;

use super::super::TokioTcpTransport;
use crate::core::{RemoteTransport, TransportBind, TransportEndpoint};

fn find_free_port() -> u16 {
  std::net::TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port()
}

#[test]
fn transport_scheme_is_fraktor_tcp() {
  let rt = Runtime::new().expect("runtime");
  let _guard = rt.enter();

  let transport = TokioTcpTransport::default();
  assert_eq!(transport.scheme(), "fraktor.tcp");
}

#[test]
fn can_spawn_listener() {
  let rt = Runtime::new().expect("runtime");
  let _guard = rt.enter();

  let transport = TokioTcpTransport::default();
  let port = find_free_port();
  let bind = TransportBind::new("127.0.0.1", Some(port));
  let handle = transport.spawn_listener(&bind);
  assert!(handle.is_ok());
}

#[test]
fn can_open_channel() {
  let rt = Runtime::new().expect("runtime");
  let _guard = rt.enter();

  let transport = TokioTcpTransport::default();
  let port = find_free_port();
  let bind = TransportBind::new("127.0.0.1", Some(port));
  let _handle = transport.spawn_listener(&bind).expect("listener");

  // リスナーが起動するまで少し待つ
  thread::sleep(Duration::from_millis(50));

  let endpoint = TransportEndpoint::new(format!("127.0.0.1:{port}"));
  let channel = transport.open_channel(&endpoint);
  assert!(channel.is_ok());
}

#[test]
fn can_send_message() {
  let rt = Runtime::new().expect("runtime");
  let _guard = rt.enter();

  let transport = TokioTcpTransport::default();
  let port = find_free_port();
  let bind = TransportBind::new("127.0.0.1", Some(port));
  let _handle = transport.spawn_listener(&bind).expect("listener");

  // リスナーが起動するまで少し待つ
  thread::sleep(Duration::from_millis(50));

  let endpoint = TransportEndpoint::new(format!("127.0.0.1:{port}"));
  let channel = transport.open_channel(&endpoint).expect("channel");

  let payload = vec![1_u8, 2, 3, 4, 5];
  let correlation = CorrelationId::from_u128(0x1234);
  let result = transport.send(&channel, &payload, correlation);
  assert!(result.is_ok());
}
