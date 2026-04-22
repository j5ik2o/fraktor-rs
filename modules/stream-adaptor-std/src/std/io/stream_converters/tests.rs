extern crate std;

use core::time::Duration;
use std::{
  io::{Cursor, Error, ErrorKind, Read, Result as IoResult, Write},
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
  time::Instant,
  vec::Vec,
};

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_stream_core_rs::core::{
  dsl::{Sink, Source},
  materialization::{
    ActorMaterializer, ActorMaterializerConfig, Completion, KeepBoth, KeepLeft, KeepRight, StreamCompletion,
  },
};

use crate::std::io::StreamConverters;

// ---------------------------------------------------------------------------
// テストハーネス
// ---------------------------------------------------------------------------

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  ActorSystem::create_with_config(&props, config).expect("system should build")
}

fn build_materializer(system: ActorSystem) -> ActorMaterializer {
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("materializer start");
  materializer
}

fn poll_completion<T: Clone + Send + 'static>(completion: &StreamCompletion<T>) -> Completion<T> {
  let deadline = Instant::now() + Duration::from_secs(5);
  loop {
    let poll = completion.poll();
    if matches!(poll, Completion::Ready(_)) {
      return poll;
    }
    if Instant::now() >= deadline {
      panic!("completion did not complete within timeout");
    }
    std::thread::yield_now();
  }
}

// ---------------------------------------------------------------------------
// 補助: テスト用 Writer/Reader 実装
// ---------------------------------------------------------------------------

/// テスト用 Writer: 受信したバイトを Vec<u8> に蓄積する。
struct RecordingWriter {
  data: Arc<Mutex<Vec<u8>>>,
}

impl Write for RecordingWriter {
  fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
    self.data.lock().expect("recording writer lock").extend_from_slice(buf);
    Ok(buf.len())
  }

  fn flush(&mut self) -> IoResult<()> {
    Ok(())
  }
}

/// テスト用 Writer: flush 呼び出し回数をカウントする。
struct FlushCountingWriter {
  data:        Arc<Mutex<Vec<u8>>>,
  flush_count: Arc<AtomicUsize>,
}

impl Write for FlushCountingWriter {
  fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
    self.data.lock().expect("flush counting writer lock").extend_from_slice(buf);
    Ok(buf.len())
  }

  fn flush(&mut self) -> IoResult<()> {
    self.flush_count.fetch_add(1, Ordering::SeqCst);
    Ok(())
  }
}

/// テスト用 Reader: 常に BrokenPipe エラーを返す。
struct FailingReader;

impl Read for FailingReader {
  fn read(&mut self, _buf: &mut [u8]) -> IoResult<usize> {
    Err(Error::new(ErrorKind::BrokenPipe, "test reader always fails"))
  }
}

// ===========================================================================
// Task J: from_output_stream テスト
// ===========================================================================

#[test]
fn from_output_stream_writes_chunks_to_writer() {
  // Given: 2 チャンク（計 6 バイト）を流すソースと、書き込み先の RecordingWriter
  let system = build_system();
  let mut materializer = build_materializer(system);

  let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
  let captured_factory = Arc::clone(&captured);

  let chunks: Vec<Vec<u8>> = std::vec![std::vec![1u8, 2, 3], std::vec![4u8, 5, 6]];
  let sink =
    StreamConverters::from_output_stream(move || RecordingWriter { data: Arc::clone(&captured_factory) }, false);
  let graph = Source::from_iterator(chunks).into_mat(sink, KeepRight);

  // When: グラフを駆動して完了まで待つ
  let materialized = graph.run(&mut materializer).expect("materialize");
  let completion = poll_completion(materialized.materialized());

  // Then: チャンクが順序通り結合されて書き込まれ、IOResult が成功を示す
  let io_result = match completion {
    | Completion::Ready(Ok(result)) => result,
    | other => panic!("expected Ready(Ok(_)) but got {other:?}"),
  };
  assert!(io_result.was_successful(), "IO result should be successful");
  assert_eq!(io_result.count(), 6, "IO result count should equal total byte count");
  let bytes = captured.lock().expect("captured lock").clone();
  assert_eq!(bytes, std::vec![1u8, 2, 3, 4, 5, 6]);
}

#[test]
fn from_output_stream_flushes_per_chunk_when_auto_flush_enabled() {
  // Given: auto_flush=true でチャンクごとに flush が呼ばれる FlushCountingWriter
  let system = build_system();
  let mut materializer = build_materializer(system);

  let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
  let flush_count: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
  let captured_factory = Arc::clone(&captured);
  let flush_count_factory = Arc::clone(&flush_count);

  let chunks: Vec<Vec<u8>> = std::vec![std::vec![1u8, 2], std::vec![3u8, 4], std::vec![5u8, 6]];
  let sink = StreamConverters::from_output_stream(
    move || FlushCountingWriter {
      data:        Arc::clone(&captured_factory),
      flush_count: Arc::clone(&flush_count_factory),
    },
    true,
  );
  let graph = Source::from_iterator(chunks).into_mat(sink, KeepRight);

  // When: 3 チャンクを流して完了まで待つ
  let materialized = graph.run(&mut materializer).expect("materialize");
  let _ = poll_completion(materialized.materialized());

  // Then: flush がチャンク数以上呼ばれている（各チャンクごと + on_complete の最終 flush）
  let count = flush_count.load(Ordering::SeqCst);
  assert!(count >= 3, "flush should be called at least once per chunk (got {count})");
  let bytes = captured.lock().expect("captured lock").clone();
  assert_eq!(bytes, std::vec![1u8, 2, 3, 4, 5, 6]);
}

#[test]
fn from_output_stream_does_not_auto_flush_per_chunk_when_disabled() {
  // Given: auto_flush=false の場合は中間 flush が呼ばれないこと
  let system = build_system();
  let mut materializer = build_materializer(system);

  let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
  let flush_count: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
  let captured_factory = Arc::clone(&captured);
  let flush_count_factory = Arc::clone(&flush_count);

  let chunks: Vec<Vec<u8>> = std::vec![std::vec![1u8, 2], std::vec![3u8, 4], std::vec![5u8, 6]];
  let sink = StreamConverters::from_output_stream(
    move || FlushCountingWriter {
      data:        Arc::clone(&captured_factory),
      flush_count: Arc::clone(&flush_count_factory),
    },
    false,
  );
  let graph = Source::from_iterator(chunks).into_mat(sink, KeepRight);

  // When: グラフを駆動して完了まで待つ
  let materialized = graph.run(&mut materializer).expect("materialize");
  let _ = poll_completion(materialized.materialized());

  // Then: flush 回数は on_complete の最終 flush の 1 回のみ（チャンクごとには呼ばれない）
  let count = flush_count.load(Ordering::SeqCst);
  assert!(count <= 1, "flush should not be called per chunk when auto_flush=false (got {count})");
}

// ===========================================================================
// Task J: from_input_stream テスト
// ===========================================================================

#[test]
fn from_input_stream_factory_is_invoked_lazily() {
  // Given: ファクトリ呼び出しを観測する AtomicBool フラグ
  let system = build_system();
  let mut materializer = build_materializer(system);

  let factory_invoked: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
  let factory_invoked_clone = Arc::clone(&factory_invoked);

  let source = StreamConverters::from_input_stream(
    move || {
      factory_invoked_clone.store(true, Ordering::SeqCst);
      Cursor::new(std::vec![1u8, 2, 3, 4])
    },
    2,
  );

  // Then: 構築直後（run の前）にはファクトリは未呼び出し
  assert!(!factory_invoked.load(Ordering::SeqCst), "factory must not be called before .run()");

  // When: グラフを駆動して完了まで待つ
  let graph = source.into_mat(Sink::ignore(), KeepLeft);
  let materialized = graph.run(&mut materializer).expect("materialize");
  let _ = poll_completion(materialized.materialized());

  // Then: run 後はファクトリが呼び出されている
  assert!(factory_invoked.load(Ordering::SeqCst), "factory must be invoked after run completes");
}

#[test]
fn from_input_stream_reports_successful_io_result_on_eof() {
  // Given: 8 バイトを chunk_size=3 で読み出す入力ストリーム
  let system = build_system();
  let mut materializer = build_materializer(system);

  let payload: Vec<u8> = std::vec![1u8, 2, 3, 4, 5, 6, 7, 8];
  let expected = payload.clone();
  let source = StreamConverters::from_input_stream(move || Cursor::new(payload), 3);

  // When: Sink::collect() と KeepBoth で IOResult とチャンク集約の両方を観測
  let graph = source.into_mat(Sink::collect(), KeepBoth);
  let materialized = graph.run(&mut materializer).expect("materialize");
  let (io_completion, chunks_completion) = materialized.materialized();

  // When: 両 completion が Ready になるまでポーリング
  let deadline = Instant::now() + Duration::from_secs(5);
  loop {
    let io_ready = matches!(io_completion.poll(), Completion::Ready(_));
    let chunks_ready = matches!(chunks_completion.poll(), Completion::Ready(_));
    if io_ready && chunks_ready {
      break;
    }
    if Instant::now() >= deadline {
      panic!("completion did not complete within timeout");
    }
    std::thread::yield_now();
  }

  // Then: IOResult は成功かつバイト数=8、チャンクを連結すると元の入力と一致
  let io_result = match io_completion.poll() {
    | Completion::Ready(Ok(result)) => result,
    | other => panic!("expected Ready(Ok(_)) for IO but got {other:?}"),
  };
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 8);

  let chunks = match chunks_completion.poll() {
    | Completion::Ready(Ok(result)) => result,
    | other => panic!("expected Ready(Ok(_)) for chunks but got {other:?}"),
  };
  let flat: Vec<u8> = chunks.into_iter().flatten().collect();
  assert_eq!(flat, expected);
}

#[test]
fn from_input_stream_reports_failed_io_result_on_read_error() {
  // Given: 常に失敗する FailingReader を返すファクトリ
  let system = build_system();
  let mut materializer = build_materializer(system);

  let source = StreamConverters::from_input_stream(|| FailingReader, 4);

  // When: Sink::ignore() と KeepLeft でソース側の IOResult を保持
  let graph = source.into_mat(Sink::ignore(), KeepLeft);
  let materialized = graph.run(&mut materializer).expect("materialize");
  let completion = poll_completion(materialized.materialized());

  // Then: IOResult は失敗を記録している
  let io_result = match completion {
    | Completion::Ready(Ok(result)) => result,
    | other => panic!("expected Ready(Ok(_)) but got {other:?}"),
  };
  assert!(!io_result.was_successful(), "IO result should record failure");
  assert!(io_result.error().is_some(), "IO result should carry error cause");
}

// ===========================================================================
// Batch 8 Task X: as_input_stream テスト（Sink<Vec<u8>, StreamInputStream>）
// ===========================================================================

#[test]
fn as_input_stream_emits_bytes_from_upstream_source_to_materialized_reader() {
  // Given: 複数チャンクを流すソースと、as_input_stream が返す Sink
  let system = build_system();
  let mut materializer = build_materializer(system);

  let chunks: Vec<Vec<u8>> = std::vec![std::vec![1u8, 2, 3], std::vec![4u8, 5], std::vec![6u8]];
  let expected: Vec<u8> = std::vec![1u8, 2, 3, 4, 5, 6];

  let sink = StreamConverters::as_input_stream(Duration::from_millis(500));
  let graph = Source::from_iterator(chunks).into_mat(sink, KeepRight);

  // When: graph を materialize して、materialized の Read 側から全バイト読み取る
  let materialized = graph.run(&mut materializer).expect("materialize");
  let mut reader = materialized.into_materialized();
  let mut buf = Vec::new();
  reader.read_to_end(&mut buf).expect("read_to_end");

  // Then: 上流のチャンクがバイト列として連結されて読める
  assert_eq!(buf, expected);
}

#[test]
fn as_input_stream_returns_eof_after_upstream_completes() {
  // Given: 1 チャンクだけを送って完了するソース
  let system = build_system();
  let mut materializer = build_materializer(system);

  let chunks: Vec<Vec<u8>> = std::vec![std::vec![42u8]];
  let sink = StreamConverters::as_input_stream(Duration::from_millis(500));
  let graph = Source::from_iterator(chunks).into_mat(sink, KeepRight);

  // When: 1 バイト読んでから次の read を試す
  let materialized = graph.run(&mut materializer).expect("materialize");
  let mut reader = materialized.into_materialized();
  let mut first = [0u8; 4];
  let n1 = reader.read(&mut first).expect("first read");
  assert_eq!(&first[..n1], &[42u8]);

  // Then: 以降の read() は EOF (Ok(0)) を返す（Pekko asInputStream と一致）
  let mut second = [0u8; 4];
  loop {
    match reader.read(&mut second) {
      | Ok(0) => break,
      | Ok(_) => continue,
      | Err(e) if e.kind() == ErrorKind::TimedOut => continue,
      | Err(e) => panic!("unexpected read error: {e:?}"),
    }
  }
}

#[test]
fn as_input_stream_read_times_out_when_upstream_does_not_emit() {
  // Given: 要素を emit しない Source::never() を as_input_stream に接続
  let system = build_system();
  let mut materializer = build_materializer(system);

  let sink = StreamConverters::as_input_stream(Duration::from_millis(50));
  let graph = Source::<Vec<u8>, _>::never().into_mat(sink, KeepRight);

  // When: materialize した reader から read を試みる
  let materialized = graph.run(&mut materializer).expect("materialize");
  let mut reader = materialized.into_materialized();
  let mut buf = [0u8; 4];
  let err = reader.read(&mut buf).expect_err("read should time out");

  // Then: ErrorKind::TimedOut（read_timeout で設定した 50ms）
  assert_eq!(err.kind(), ErrorKind::TimedOut);
}

// ===========================================================================
// Batch 8 Task X: as_output_stream テスト（Source<Vec<u8>, StreamOutputStream>）
// ===========================================================================

#[test]
fn as_output_stream_emits_bytes_written_via_materialized_writer() {
  // Given: as_output_stream で生成したソースを RecordingWriter へ書き込む Sink に接続
  let system = build_system();
  let mut materializer = build_materializer(system);

  let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
  let captured_factory = Arc::clone(&captured);

  let source = StreamConverters::as_output_stream(Duration::from_millis(500));
  let sink =
    StreamConverters::from_output_stream(move || RecordingWriter { data: Arc::clone(&captured_factory) }, false);
  let graph = source.into_mat(sink, KeepLeft);

  // When: materialize した writer に 3 回書き込み、drop して stream を完了させる
  let materialized = graph.run(&mut materializer).expect("materialize");
  let mut writer = materialized.into_materialized();
  writer.write(&[7u8, 8]).expect("write 1");
  writer.write(&[9u8]).expect("write 2");
  writer.write(&[10u8, 11, 12]).expect("write 3");
  drop(writer);

  // Then: RecordingWriter に 6 バイト連結されて書き込まれている（Pekko asOutputStream と一致）
  let deadline = Instant::now() + Duration::from_secs(5);
  loop {
    let len = captured.lock().expect("captured lock").len();
    if len >= 6 {
      break;
    }
    if Instant::now() >= deadline {
      panic!("captured data did not reach expected size");
    }
    std::thread::yield_now();
  }
  let bytes = captured.lock().expect("captured lock").clone();
  assert_eq!(bytes, std::vec![7u8, 8, 9, 10, 11, 12]);
}

#[test]
fn as_output_stream_completes_source_when_writer_is_dropped() {
  // Given: as_output_stream で生成したソースと RecordingWriter Sink
  let system = build_system();
  let mut materializer = build_materializer(system);

  let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
  let captured_factory = Arc::clone(&captured);
  let flush_count: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
  let flush_count_factory = Arc::clone(&flush_count);

  let source = StreamConverters::as_output_stream(Duration::from_millis(500));
  let sink = StreamConverters::from_output_stream(
    move || FlushCountingWriter {
      data:        Arc::clone(&captured_factory),
      flush_count: Arc::clone(&flush_count_factory),
    },
    false,
  );
  // KeepBoth で「Source 側の writer」と「Sink 側の IOResult completion」を同時に観測
  let graph = source.into_mat(sink, KeepBoth);
  let materialized = graph.run(&mut materializer).expect("materialize");
  let (mut writer, io_completion) = materialized.into_materialized();

  // When: 1 回だけ書いて writer を drop
  writer.write(&[1u8, 2, 3]).expect("write");
  drop(writer);

  // Then: Sink 側の IOResult が成功で完了する（writer drop → source 完了 → sink on_complete）
  let completion = poll_completion(&io_completion);
  match completion {
    | Completion::Ready(Ok(result)) => assert!(result.was_successful(), "IO result should be successful"),
    | other => panic!("expected Ready(Ok(_)) but got {other:?}"),
  }
  let bytes = captured.lock().expect("captured lock").clone();
  assert_eq!(bytes, std::vec![1u8, 2, 3]);
}

#[test]
fn as_output_stream_write_times_out_when_downstream_does_not_consume() {
  // Given: as_output_stream と Sink::ignore() を接続するが、materialization 前に write
  // を失敗させるため        downstream 消費を遅延させるケースを作る。ここでは write_timeout=50ms
  // を短く設定し、        capacity 上限（固定 16）を埋めきるまで複数回 write を試みる。
  let system = build_system();
  let mut materializer = build_materializer(system);

  // 消費側をブロックする Sink を組む（cancelled は cancel を発行するため不適切、代わりに
  // 1 要素だけ read してその後永久にブロックする fold を用意すると煩雑なため、
  // ここでは as_input_stream を接続して reader を読まないことで消費を止める）。
  let source = StreamConverters::as_output_stream(Duration::from_millis(50));
  let sink = StreamConverters::as_input_stream(Duration::from_secs(60));
  let graph = source.into_mat(sink, KeepBoth);

  let materialized = graph.run(&mut materializer).expect("materialize");
  let (mut writer, _reader) = materialized.into_materialized();

  // When: capacity=16 を超えて書き込み、downstream (as_input_stream 側) は reader
  // を読まないため満杯になる
  let mut saw_timeout = false;
  for _ in 0..64 {
    match writer.write(&[0u8; 256]) {
      | Ok(_) => continue,
      | Err(err) if err.kind() == ErrorKind::TimedOut => {
        saw_timeout = true;
        break;
      },
      | Err(err) => panic!("unexpected write error: {err:?}"),
    }
  }

  // Then: いずれかの反復で TimedOut が観測される
  assert!(saw_timeout, "write should eventually time out when downstream does not consume");
}
