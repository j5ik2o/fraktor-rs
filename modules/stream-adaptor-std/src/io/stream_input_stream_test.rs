extern crate std;

use core::time::Duration;
use std::{
  io::{ErrorKind, Read},
  sync::mpsc::{SyncSender, sync_channel},
  vec::Vec,
};

use crate::io::StreamInputStream;

// ---------------------------------------------------------------------------
// Batch 8 Task X: StreamInputStream — std::io::Read 実装ユニットテスト
// ---------------------------------------------------------------------------
//
// これらのテストは `StreamInputStream::from_channel(receiver, read_timeout)`
// 的なコンストラクタを経由して Read 側の挙動を直接検証する。sync_channel の
// Sender 側が切断された場合の EOF 応答、read_timeout 経過後の TimedOut、
// 残バッファの分割読み出しなど、stage 側をまたがない Rust 境界単体仕様を固定する。

/// ヘルパ: タイムアウト 100ms の StreamInputStream と sender を組で返す。
fn build_input_stream_with_sender(read_timeout: Duration) -> (StreamInputStream, SyncSender<Vec<u8>>) {
  let (sender, receiver) = sync_channel::<Vec<u8>>(16);
  let stream = StreamInputStream::from_channel(receiver, read_timeout);
  (stream, sender)
}

#[test]
fn stream_input_stream_reads_bytes_sent_from_upstream_channel() {
  // Given: 16-capacity sync_channel で上流からチャンクを送信できる StreamInputStream
  let (mut stream, sender) = build_input_stream_with_sender(Duration::from_millis(100));
  sender.send(std::vec![1u8, 2, 3, 4]).expect("send");

  // When: read() を十分なバッファで呼ぶ
  let mut buf = [0u8; 8];
  let n = stream.read(&mut buf).expect("read should succeed");

  // Then: 上流からのチャンクがバイト単位で読み取れる
  assert_eq!(n, 4);
  assert_eq!(&buf[..n], &[1u8, 2, 3, 4]);
}

#[test]
fn stream_input_stream_read_returns_eof_when_sender_is_dropped() {
  // Given: sender を即 drop すると channel は disconnected 状態になる
  let (mut stream, sender) = build_input_stream_with_sender(Duration::from_millis(50));
  drop(sender);

  // When: read() を呼ぶ
  let mut buf = [0u8; 4];
  let n = stream.read(&mut buf).expect("read should succeed with EOF");

  // Then: 0 を返し EOF を示す（Pekko asInputStream のセマンティクスと一致）
  assert_eq!(n, 0);
}

#[test]
fn stream_input_stream_read_returns_timed_out_when_no_data_arrives() {
  // Given: sender は保持したまま何も送らず、read_timeout が経過するのを待つ
  let (mut stream, _sender) = build_input_stream_with_sender(Duration::from_millis(50));

  // When: read() を呼ぶ
  let mut buf = [0u8; 4];
  let err = stream.read(&mut buf).expect_err("read should time out");

  // Then: ErrorKind::TimedOut
  assert_eq!(err.kind(), ErrorKind::TimedOut);
}

#[test]
fn stream_input_stream_splits_chunk_larger_than_buffer() {
  // Given: 8 バイトのチャンクを送り、buf は 3 バイト
  let (mut stream, sender) = build_input_stream_with_sender(Duration::from_millis(100));
  sender.send(std::vec![10u8, 20, 30, 40, 50, 60, 70, 80]).expect("send");

  // When: 3 バイト buf で繰り返し read する
  let mut buf = [0u8; 3];
  let n1 = stream.read(&mut buf).expect("read 1");
  let chunk_1: Vec<u8> = buf[..n1].to_vec();
  let n2 = stream.read(&mut buf).expect("read 2");
  let chunk_2: Vec<u8> = buf[..n2].to_vec();
  let n3 = stream.read(&mut buf).expect("read 3");
  let chunk_3: Vec<u8> = buf[..n3].to_vec();

  // Then: 3+3+2 バイトに分割して順序どおり返る（残バッファ挙動）
  assert_eq!(chunk_1, std::vec![10u8, 20, 30]);
  assert_eq!(chunk_2, std::vec![40u8, 50, 60]);
  assert_eq!(chunk_3, std::vec![70u8, 80]);
}

#[test]
fn stream_input_stream_drains_remaining_buffer_before_waiting_on_channel() {
  // Given: 5 バイトのチャンクを 3 バイト buf で読んで 2 バイト残す
  let (mut stream, sender) = build_input_stream_with_sender(Duration::from_millis(100));
  sender.send(std::vec![1u8, 2, 3, 4, 5]).expect("send");

  let mut buf = [0u8; 3];
  let _ = stream.read(&mut buf).expect("first read");
  // sender は既に drop してもよい（残バッファが先に返るはず）
  drop(sender);

  // When: 次の read を呼ぶ（buf は 4 バイト確保）
  let mut buf2 = [0u8; 4];
  let n = stream.read(&mut buf2).expect("drain read");

  // Then: 残バッファから 2 バイト返る（channel はまだ照会しない）
  assert_eq!(&buf2[..n], &[4u8, 5]);
}
