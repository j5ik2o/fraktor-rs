use std::io::Write;

use tempfile::NamedTempFile;

use crate::{
  core::{
    Completion, KeepBoth, KeepRight, StreamBufferConfig, StreamCompletion, StreamError,
    lifecycle::{Stream, StreamHandleId, StreamHandleImpl, StreamShared},
    mat::{Materialized, Materializer, RunnableGraph},
    stage::{Sink, Source},
  },
  std::file_io::FileIO,
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

fn drive_to_completion<Mat>(materialized: &Materialized<Mat>) {
  for _ in 0..256 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      return;
    }
  }
  panic!("stream did not reach terminal state after 256 drive iterations");
}

// --- from_path tests ---

#[test]
fn from_path_reads_file_contents() {
  let mut tmp = NamedTempFile::new().unwrap();
  tmp.write_all(b"hello").unwrap();
  tmp.flush().unwrap();

  let source = FileIO::from_path(tmp.path());
  let graph = source.to_mat(Sink::<u8, StreamCompletion<alloc::vec::Vec<u8>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  let (io_result, completion) = materialized.materialized();
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 5);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![b'h', b'e', b'l', b'l', b'o'])));
}

#[test]
fn from_path_nonexistent_returns_failed_io_result() {
  let source = FileIO::from_path("/nonexistent/path/to/file.txt");
  let graph = source.to_mat(Sink::<u8, StreamCompletion<alloc::vec::Vec<u8>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  let (io_result, _completion) = materialized.materialized();
  assert!(!io_result.was_successful());
  assert_eq!(io_result.count(), 0);
}

#[test]
fn from_path_empty_file_returns_zero_count() {
  let tmp = NamedTempFile::new().unwrap();

  let source = FileIO::from_path(tmp.path());
  let graph = source.to_mat(Sink::<u8, StreamCompletion<alloc::vec::Vec<u8>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  let (io_result, completion) = materialized.materialized();
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 0);
  assert_eq!(completion.poll(), Completion::Ready(Ok(alloc::vec::Vec::<u8>::new())));
}

// --- to_path tests ---

#[test]
fn to_path_writes_file_contents() {
  let tmp = NamedTempFile::new().unwrap();
  let path = tmp.path().to_path_buf();

  let source = Source::from_iterator(vec![b'w', b'o', b'r', b'l', b'd']);
  let sink = FileIO::to_path(&path);
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(io_result.was_successful());
      assert_eq!(io_result.count(), 5);
    },
    | other => panic!("expected Ready(Ok(IOResult)), got {other:?}"),
  }

  let written = std::fs::read(&path).unwrap();
  assert_eq!(written, b"world");
}

#[test]
fn to_path_empty_stream_writes_empty_file() {
  let tmp = NamedTempFile::new().unwrap();
  let path = tmp.path().to_path_buf();

  let source = Source::<u8, _>::empty();
  let sink = FileIO::to_path(&path);
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(io_result.was_successful());
      assert_eq!(io_result.count(), 0);
    },
    | other => panic!("expected Ready(Ok(IOResult)), got {other:?}"),
  }

  let written = std::fs::read(&path).unwrap();
  assert!(written.is_empty());
}

#[test]
fn to_path_invalid_directory_returns_failed_io_result() {
  let source = Source::from_iterator(vec![1_u8, 2, 3]);
  let sink = FileIO::to_path("/nonexistent/directory/file.txt");
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(!io_result.was_successful());
      assert_eq!(io_result.count(), 0);
    },
    | other => panic!("expected Ready(Ok(IOResult::failed)), got {other:?}"),
  }
}
