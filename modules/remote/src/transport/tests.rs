#![cfg(test)]

use std::sync::{Arc, Mutex};

use fraktor_actor_rs::core::event_stream::BackpressureSignal;

use crate::{
  LoopbackTransport,
  RemoteTransport,
  RemotingError,
  RemotingExtensionConfig,
  TransportBind,
  TransportEndpoint,
  TransportFactory,
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

#[test]
fn loopback_transport_frames_payload_with_length_prefix() {
  let transport = LoopbackTransport::new();
  let handle = <LoopbackTransport as RemoteTransport<NoStdToolbox>>::spawn_listener(
    &transport,
    &TransportBind::new("local"),
  )
  .expect("listener");
  let channel = <LoopbackTransport as RemoteTransport<NoStdToolbox>>::open_channel(
    &transport,
    &TransportEndpoint::new("local"),
  )
  .expect("channel");

  <LoopbackTransport as RemoteTransport<NoStdToolbox>>::send(&transport, &channel, b"ping")
    .expect("send");
  let frames = handle.take_frames();
  assert_eq!(frames.len(), 1);
  let frame = &frames[0];
  assert!(frame.len() >= 4);
  let mut len_bytes = [0u8; 4];
  len_bytes.copy_from_slice(&frame[..4]);
  let len = u32::from_be_bytes(len_bytes) as usize;
  assert_eq!(len, frame.len() - 4);
  assert_eq!(&frame[4..], b"ping");
}

#[test]
fn loopback_transport_invokes_backpressure_hook() {
  use core::sync::atomic::{AtomicUsize, Ordering};

  let transport = LoopbackTransport::new();
  let counter = Arc::new(AtomicUsize::new(0));
  let authority = "bp";
  let recordings = Arc::new(Mutex::new(Vec::new()));
  let tx_counter = counter.clone();
  let recordings_clone = recordings.clone();
  <LoopbackTransport as RemoteTransport<NoStdToolbox>>::install_backpressure_hook(
    &transport,
    Arc::new(move |signal, auth| {
      tx_counter.fetch_add(1, Ordering::Relaxed);
      recordings_clone.lock().unwrap().push((signal, auth.to_string()));
    }),
  );

  let handle = <LoopbackTransport as RemoteTransport<NoStdToolbox>>::spawn_listener(
    &transport,
    &TransportBind::new(authority),
  )
  .expect("listener");
  let channel = <LoopbackTransport as RemoteTransport<NoStdToolbox>>::open_channel(
    &transport,
    &TransportEndpoint::new(authority),
  )
  .expect("channel");

  for _ in 0..10 {
    <LoopbackTransport as RemoteTransport<NoStdToolbox>>::send(
      &transport,
      &channel,
      b"payload",
    )
    .expect("send");
  }

  assert!(counter.load(Ordering::Relaxed) > 0);
  assert!(recordings.lock().unwrap().iter().any(|(signal, auth)| *signal == BackpressureSignal::Apply && auth == authority));

  let _ = handle.take_frames();
  <LoopbackTransport as RemoteTransport<NoStdToolbox>>::send(
    &transport,
    &channel,
    b"resume",
  )
  .expect("send");
  assert!(recordings
    .lock()
    .unwrap()
    .iter()
    .any(|(signal, auth)| *signal == BackpressureSignal::Release && auth == authority));
}

#[test]
fn transport_factory_rejects_unknown_scheme() {
  let config = RemotingExtensionConfig::default().with_transport_scheme("fraktor.unknown");
  match TransportFactory::create::<NoStdToolbox>(&config) {
    | Ok(_) => panic!("expected error"),
    | Err(err) => assert!(matches!(err, RemotingError::TransportUnavailable(_))),
  }
}
