use alloc::vec::Vec;
use std::thread;

use super::{RunnableGraph, Sink, Source, TailSource};
use crate::core::{
  StreamError,
  r#impl::{
    fusing::StreamBufferConfig,
    interpreter::{DEFAULT_BOUNDARY_CAPACITY, IslandBoundaryShared, IslandSplitter},
    materialization::{Stream, StreamShared},
  },
  materialization::{DriveOutcome, Materialized, Materializer},
};

#[derive(Default)]
struct TestMaterializer {
  streams: Vec<StreamShared>,
}

impl TestMaterializer {
  const DRIVE_LIMIT: usize = 4096;

  fn drive_until_terminal(&self) -> Result<(), StreamError> {
    let mut idle_budget = Self::DRIVE_LIMIT;
    while self.streams.iter().any(|stream| !stream.state().is_terminal()) {
      let mut progressed = false;
      for stream in &self.streams {
        if !stream.state().is_terminal() && matches!(stream.drive(), DriveOutcome::Progressed) {
          progressed = true;
        }
      }
      if progressed {
        idle_budget = Self::DRIVE_LIMIT;
      } else if idle_budget == 0 {
        return Err(StreamError::WouldBlock);
      } else {
        thread::yield_now();
        idle_budget = idle_budget.saturating_sub(1);
      }
    }
    Ok(())
  }
}

impl Materializer for TestMaterializer {
  fn start(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError> {
    let (plan, materialized) = graph.into_parts();
    let island_plan = IslandSplitter::split(plan);

    if island_plan.islands().len() <= 1 {
      let mut stream = Stream::new(island_plan.into_single_plan(), StreamBufferConfig::default());
      stream.start()?;
      let stream = StreamShared::new(stream);
      self.streams.push(stream.clone());
      return Ok(Materialized::new(stream, materialized));
    }

    let (mut islands, crossings) = island_plan.into_parts();
    for crossing in crossings {
      let upstream_idx = crossing.from_island().as_usize();
      let downstream_idx = crossing.to_island().as_usize();
      let boundary_capacity = islands[downstream_idx]
        .input_buffer_capacity_for_inlet(crossing.to_port())
        .unwrap_or(DEFAULT_BOUNDARY_CAPACITY);
      let boundary = IslandBoundaryShared::new(boundary_capacity);
      islands[upstream_idx].add_boundary_sink(boundary.clone(), crossing.from_port(), crossing.element_type());
      islands[downstream_idx].add_boundary_source(boundary, crossing.to_port(), crossing.element_type());
    }

    let mut streams = Vec::with_capacity(islands.len());
    for island in islands {
      let mut stream = Stream::new(island.into_stream_plan(), StreamBufferConfig::default());
      stream.start()?;
      streams.push(StreamShared::new(stream));
    }

    let stream = streams.first().cloned().ok_or(StreamError::Failed)?;
    self.streams.extend(streams);
    Ok(Materialized::new(stream, materialized))
  }

  fn shutdown(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

pub(crate) trait RunWithCollectSink<Out> {
  fn run_with_collect_sink(self) -> Result<Vec<Out>, StreamError>;
}

impl<Out, Mat> RunWithCollectSink<Out> for Source<Out, Mat>
where
  Out: Send + Sync + 'static,
{
  fn run_with_collect_sink(self) -> Result<Vec<Out>, StreamError> {
    let mut materializer = TestMaterializer::default();
    let materialized = self.run_with(Sink::collect(), &mut materializer)?;
    materializer.drive_until_terminal()?;
    materialized.materialized().try_take().unwrap_or(Err(StreamError::Failed))
  }
}

impl<Out> RunWithCollectSink<Out> for TailSource<Out>
where
  Out: Send + Sync + 'static,
{
  fn run_with_collect_sink(self) -> Result<Vec<Out>, StreamError> {
    self.into_source().run_with_collect_sink()
  }
}
