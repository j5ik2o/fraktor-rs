#![feature(register_tool)]
#![register_tool(ambiguous_suffix)]
#![warn(ambiguous_suffix::ambiguous_suffix)]

pub struct TaskRegistry;

pub enum ConnectionPolicy {
  Http,
  Grpc,
}

pub trait DataFormatter {
  fn format(&self) -> String;
}

pub struct SessionCoordinator;

pub struct EventDispatcher;

pub struct QueryExecutor;

// private types are allowed to use any suffix
struct InternalManager;

fn main() {}
