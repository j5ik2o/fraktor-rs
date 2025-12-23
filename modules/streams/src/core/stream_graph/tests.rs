use crate::core::{Inlet, MatCombine, Outlet, Source, StageKind, StreamGraph};

impl StreamGraph {
  fn stage_kinds(&self) -> Vec<StageKind> {
    self.stages.iter().map(super::StageDefinition::kind).collect()
  }

  fn stage_mat_combines(&self) -> Vec<MatCombine> {
    self.stages.iter().map(super::StageDefinition::mat_combine).collect()
  }

  fn connection_count(&self) -> usize {
    self.connections.len()
  }

  fn connections(&self) -> Vec<(super::PortId, super::PortId, MatCombine)> {
    self.connections.iter().map(|conn| (conn.from, conn.to, conn.mat)).collect()
  }
}

#[test]
fn connect_rejects_unknown_ports() {
  let mut graph = StreamGraph::new();
  let result = graph.connect(&Outlet::<u32>::new(), &Inlet::<u32>::new(), MatCombine::KeepLeft);
  assert!(result.is_err());
}

#[test]
fn graph_tracks_stage_metadata() {
  let source = Source::single(1_u32).map(|value| value + 1);
  let (graph, _mat) = source.into_parts();
  assert_eq!(graph.stage_kinds(), vec![StageKind::SourceSingle, StageKind::FlowMap]);
  assert_eq!(graph.stage_mat_combines(), vec![MatCombine::KeepRight, MatCombine::KeepLeft]);
  assert_eq!(graph.connection_count(), 1);
  assert_eq!(graph.connections().len(), 1);
}
