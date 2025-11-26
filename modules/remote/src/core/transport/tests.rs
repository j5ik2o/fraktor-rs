#![cfg(any(test, feature = "test-support"))]

use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdMutex, sync::ArcShared};

use super::{
  backpressure_hook::{TransportBackpressureHook, TransportBackpressureHookShared},
  factory::TransportFactory,
  loopback_transport::LoopbackTransport,
  remote_transport::RemoteTransport,
  transport_bind::TransportBind,
  transport_endpoint::TransportEndpoint,
  transport_error::TransportError,
};
use crate::core::remoting_extension_config::RemotingExtensionConfig;

#[test]
fn factory_resolves_loopback_scheme() {
  let config = RemotingExtensionConfig::default().with_transport_scheme("fraktor.loopback");
  let transport = TransportFactory::build(&config).expect("transport resolved");
  assert_eq!(transport.scheme(), "fraktor.loopback");
}

#[test]
fn factory_rejects_unknown_scheme() {
  let config = RemotingExtensionConfig::default().with_transport_scheme("fraktor.invalid");
  match TransportFactory::build(&config) {
    | Ok(_) => panic!("expected unsupported scheme"),
    | Err(error) => match error {
      | TransportError::UnsupportedScheme(scheme) => assert_eq!(scheme, "fraktor.invalid"),
      | other => panic!("unexpected error: {other:?}"),
    },
  }
}

#[test]
fn loopback_frames_include_length_and_correlation() {
  let transport = LoopbackTransport::default();
  let bind = TransportBind::new("127.0.0.1", Some(4100));
  let handle = transport.spawn_listener(&bind).expect("listener");
  let endpoint = TransportEndpoint::new("127.0.0.1:4100".into());
  let channel = transport.open_channel(&endpoint).expect("channel");
  let payload = vec![1_u8, 2, 3, 4];
  let correlation = CorrelationId::from_u128(0xAA55);
  transport.send(&channel, &payload, correlation).expect("send succeeds");

  let frames = transport.drain_frames_for_test(&handle);
  assert_eq!(frames.len(), 1);
  let frame = &frames[0];
  assert_eq!(frame.len(), 4 + 12 + payload.len());
  let mut len_bytes = [0_u8; 4];
  len_bytes.copy_from_slice(&frame[..4]);
  let length = u32::from_be_bytes(len_bytes);
  assert_eq!(length as usize, 12 + payload.len());
  assert_eq!(&frame[4..16], &correlation.to_be_bytes());
  assert_eq!(&frame[16..], &payload);
}

#[test]
fn loopback_backpressure_hook_triggers_listener() {
  struct RecordingHook;
  impl TransportBackpressureHook for RecordingHook {
    fn on_backpressure(&mut self, signal: BackpressureSignal, authority: &str, correlation_id: CorrelationId) {
      assert_eq!(authority, "loopback:test");
      assert_eq!(signal, BackpressureSignal::Apply);
      assert_eq!(correlation_id, CorrelationId::from_u128(42));
    }
  }

  let transport = LoopbackTransport::default();
  let hook: TransportBackpressureHookShared = ArcShared::new(NoStdMutex::new(Box::new(RecordingHook)));
  transport.install_backpressure_hook(hook);
  transport.emit_backpressure_for_test("loopback:test", BackpressureSignal::Apply, CorrelationId::from_u128(42));
}
