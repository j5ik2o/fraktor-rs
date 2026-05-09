use fraktor_actor_core_rs::actor::ActorCell;

fn probe(cell: &ActorCell) {
  let _ = cell.acquire_adapter_handle();
}

fn main() {
  let _ = probe;
}
