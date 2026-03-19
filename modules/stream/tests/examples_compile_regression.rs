#[path = "../examples/custom_graph_stage_std/main.rs"]
mod custom_graph_stage_std;

#[test]
fn custom_graph_stage_example_should_run_after_stage_context_contract_changes() {
  custom_graph_stage_std::main();
}
