use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use crate::core::{
  Completion, KeepRight, StreamBufferConfig, StreamCompletion, StreamDslError, StreamError,
  lifecycle::{Stream, StreamHandleId, StreamHandleImpl, StreamShared},
  mat::{Materialized, Materializer, RunnableGraph},
  queue::QueueOfferResult,
  stage::{Sink, Source},
};

struct TestMaterializer;

impl Materializer for TestMaterializer {
  fn start(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError> {
    let (plan, materialized) = graph.into_parts();
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    let shared = StreamShared::new(stream);
    let handle = StreamHandleImpl::new(StreamHandleId::next(), shared);
    Ok(Materialized::new(handle, materialized))
  }

  fn shutdown(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

// Source::create はバックグラウンドスレッドでプロデューサーを実行するため、
// WouldBlock 中にスレッドへ実行機会を与えるよう yield_now() を挟む。
const MAX_DRIVE_ITERATIONS: usize = 100_000;

fn drive_to_completion<Mat>(materialized: &Materialized<Mat>) {
  for _ in 0..MAX_DRIVE_ITERATIONS {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      return;
    }
    std::thread::yield_now();
  }
  panic!("ストリームが {MAX_DRIVE_ITERATIONS} 回の drive で終了状態に達しなかった");
}

#[test]
fn create_rejects_zero_capacity() {
  // 準備: capacity=0 は不正引数
  let result = Source::<u32, _>::create(0, |_queue| {});
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "capacity", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn create_producer_sends_elements_and_completes() {
  // 準備: 3 つの値を送信してリターンするプロデューサー
  let source = Source::create(8, |mut queue| {
    assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);
    assert_eq!(queue.offer(2_u32), QueueOfferResult::Enqueued);
    assert_eq!(queue.offer(3_u32), QueueOfferResult::Enqueued);
  })
  .expect("create");

  let graph = source.to_mat(Sink::<u32, StreamCompletion<alloc::vec::Vec<u32>>>::collect(), KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // 検証: 3 つの値が順序通りに collect される
  assert_eq!(
    materialized.materialized().poll(),
    Completion::Ready(Ok(alloc::vec![1_u32, 2, 3]))
  );
}

#[test]
fn create_producer_empty_completes_stream() {
  // 準備: 何も送信しないプロデューサー
  let source = Source::<u32, _>::create(8, |_queue| {}).expect("create");

  let graph = source.to_mat(Sink::<u32, StreamCompletion<alloc::vec::Vec<u32>>>::collect(), KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // 検証: 空のコレクションで正常完了
  assert_eq!(
    materialized.materialized().poll(),
    Completion::Ready(Ok(alloc::vec::Vec::<u32>::new()))
  );
}

#[test]
fn create_producer_panic_fails_stream() {
  // 準備: パニックを起こすプロデューサー（catch_unwind で捕捉される）
  let source = Source::<u32, _>::create(8, |_queue| {
    panic!("test panic");
  })
  .expect("create");

  let graph = source.to_mat(Sink::<u32, StreamCompletion<alloc::vec::Vec<u32>>>::collect(), KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // 検証: StreamError::Failed でストリームが失敗する
  assert!(matches!(
    materialized.materialized().poll(),
    Completion::Ready(Err(StreamError::Failed))
  ));
}

#[test]
fn create_producer_starts_lazily() {
  // 準備: 呼び出しを記録するプロデューサー
  let called = Arc::new(AtomicBool::new(false));
  let source = Source::<u32, _>::create(8, {
    let called = called.clone();
    move |_queue| {
      called.store(true, Ordering::SeqCst);
    }
  })
  .expect("create");

  // create 直後はプロデューサーが呼ばれていない
  assert!(!called.load(Ordering::SeqCst));

  let graph = source.to_mat(Sink::<u32, StreamCompletion<alloc::vec::Vec<u32>>>::collect(), KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // drive 後にはプロデューサーが呼ばれている
  assert!(called.load(Ordering::SeqCst));
}
