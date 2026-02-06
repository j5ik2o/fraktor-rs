#![feature(register_tool)]
#![register_tool(ambiguous_suffix)]
#![warn(ambiguous_suffix::ambiguous_suffix)]

pub struct TaskManager;

pub enum ConnectionService {
  Http,
  Grpc,
}

pub trait DataUtil {
  fn process(&self);
}

pub struct SessionFacade;

pub struct EventRuntime;

pub struct QueryEngine;

fn main() {}
