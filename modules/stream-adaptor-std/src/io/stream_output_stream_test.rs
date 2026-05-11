extern crate std;

use core::time::Duration;
use std::{
  io::{ErrorKind, Write},
  sync::mpsc::{Receiver, sync_channel},
  vec::Vec,
};

use crate::io::StreamOutputStream;

// ---------------------------------------------------------------------------
// Batch 8 Task X: StreamOutputStream — std::io::Write 実装ユニットテスト
// ---------------------------------------------------------------------------
//
// `StreamOutputStream::from_channel(sender, write_timeout)` を経由して Write
// 側の挙動を直接検証する。sync_channel の Receiver 側が切断された場合の
// BrokenPipe、write_timeout 経過後の TimedOut、flush が no-op であることなど
// を固定する。

fn build_output_stream_with_receiver(
  write_timeout: Duration,
  capacity: usize,
) -> (StreamOutputStream, Receiver<Vec<u8>>) {
  let (sender, receiver) = sync_channel::<Vec<u8>>(capacity);
  let stream = StreamOutputStream::from_channel(sender, write_timeout);
  (stream, receiver)
}

#[test]
fn stream_output_stream_write_forwards_chunk_to_downstream_channel() {
  // Given: capacity=16 の sync_channel と StreamOutputStream
  let (mut stream, receiver) = build_output_stream_with_receiver(Duration::from_millis(100), 16);

  // When: 4 バイト書き込む
  let written = stream.write(&[1u8, 2, 3, 4]).expect("write should succeed");

  // Then: 戻り値は buf.len() と等しく、receiver 側で同一の Vec<u8> が受信可能
  assert_eq!(written, 4);
  let chunk = receiver.recv_timeout(Duration::from_millis(100)).expect("recv");
  assert_eq!(chunk, std::vec![1u8, 2, 3, 4]);
}

#[test]
fn stream_output_stream_write_returns_broken_pipe_when_receiver_is_dropped() {
  // Given: receiver を即 drop すると channel は disconnected になる
  let (mut stream, receiver) = build_output_stream_with_receiver(Duration::from_millis(50), 1);
  drop(receiver);

  // When: write() を呼ぶ
  let err = stream.write(&[1u8, 2, 3]).expect_err("write should fail");

  // Then: ErrorKind::BrokenPipe
  assert_eq!(err.kind(), ErrorKind::BrokenPipe);
}

#[test]
fn stream_output_stream_write_returns_timed_out_when_channel_is_full() {
  // Given: capacity=1 の channel に既に 1 件積んで満杯にする
  let (mut stream, _receiver) = build_output_stream_with_receiver(Duration::from_millis(50), 1);
  stream.write(&[1u8, 2, 3]).expect("first write fills the channel");

  // When: 満杯状態で追加書き込みを試みる（受信側はブロックされたまま）
  let err = stream.write(&[4u8, 5, 6]).expect_err("write should time out");

  // Then: ErrorKind::TimedOut
  assert_eq!(err.kind(), ErrorKind::TimedOut);
}

#[test]
fn stream_output_stream_flush_is_noop() {
  // Given: sync_channel には明示的なバッファ flush 概念がない
  let (mut stream, _receiver) = build_output_stream_with_receiver(Duration::from_millis(50), 1);

  // When: flush を呼ぶ
  let result = stream.flush();

  // Then: エラーなく返る（no-op）
  assert!(result.is_ok());
}

#[test]
fn stream_output_stream_write_all_forwards_multiple_writes_sequentially() {
  // Given: capacity=16、十分な receive 余地がある
  let (mut stream, receiver) = build_output_stream_with_receiver(Duration::from_millis(100), 16);

  // When: 3 回続けて write する
  stream.write(&[1u8, 2]).expect("write 1");
  stream.write(&[3u8, 4]).expect("write 2");
  stream.write(&[5u8, 6]).expect("write 3");

  // Then: 各 write は独立した Vec<u8> チャンクとして受信される
  let c1 = receiver.recv_timeout(Duration::from_millis(100)).expect("recv 1");
  let c2 = receiver.recv_timeout(Duration::from_millis(100)).expect("recv 2");
  let c3 = receiver.recv_timeout(Duration::from_millis(100)).expect("recv 3");
  assert_eq!(c1, std::vec![1u8, 2]);
  assert_eq!(c2, std::vec![3u8, 4]);
  assert_eq!(c3, std::vec![5u8, 6]);
}

#[test]
fn stream_output_stream_drop_closes_sender_side_of_channel() {
  // Given: StreamOutputStream を drop すると内部 sender も drop される
  let (stream, receiver) = build_output_stream_with_receiver(Duration::from_millis(50), 1);
  drop(stream);

  // When: receiver.recv() を呼ぶ
  let err = receiver.recv().expect_err("recv should fail after sender drop");

  // Then: recv は disconnected を検出する（Pekko の OutputStream 終端セマンティクスと一致）
  let _ = err; // 型: std::sync::mpsc::RecvError — disconnected の存在のみが重要
}
