//! Custom `GraphStage` example that runs stage logic against a lightweight local context.

use fraktor_streams_rs::core::{
  StreamError, StreamNotUsed,
  graph::{GraphStage, GraphStageLogic},
  shape::{Inlet, Outlet, StreamShape},
  stage::StageContext,
};

struct MultiplyStage {
  factor: u32,
}

impl MultiplyStage {
  const fn new(factor: u32) -> Self {
    Self { factor }
  }
}

impl GraphStage<u32, u32, StreamNotUsed> for MultiplyStage {
  fn shape(&self) -> StreamShape<u32, u32> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<u32, u32, StreamNotUsed>> {
    Box::new(MultiplyStageLogic { factor: self.factor })
  }
}

struct MultiplyStageLogic {
  factor: u32,
}

impl GraphStageLogic<u32, u32, StreamNotUsed> for MultiplyStageLogic {
  fn on_push(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let input = ctx.grab();
    ctx.push(input * self.factor);
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

struct DemoStageContext {
  input:    Option<u32>,
  output:   Option<u32>,
  pulled:   bool,
  complete: bool,
  failed:   Option<StreamError>,
}

impl DemoStageContext {
  const fn with_input(input: u32) -> Self {
    Self { input: Some(input), output: None, pulled: false, complete: false, failed: None }
  }
}

impl StageContext<u32, u32> for DemoStageContext {
  fn pull(&mut self) {
    self.pulled = true;
  }

  fn grab(&mut self) -> u32 {
    self.input.take().expect("input must be present before on_push")
  }

  fn push(&mut self, out: u32) {
    self.output = Some(out);
  }

  fn complete(&mut self) {
    self.complete = true;
  }

  fn fail(&mut self, error: StreamError) {
    self.failed = Some(error);
  }
}

fn main() {
  let stage = MultiplyStage::new(2);
  let _shape = stage.shape();
  let mut logic = stage.create_logic();
  let mut context = DemoStageContext::with_input(21);

  logic.on_start(&mut context);
  logic.on_push(&mut context);
  let _ = logic.materialized();

  match (context.output, context.failed) {
    | (Some(value), None) => println!("custom graph stage output: {value}"),
    | (_, Some(error)) => println!("custom graph stage failed: {error}"),
    | _ => println!("custom graph stage produced no output"),
  }
}
