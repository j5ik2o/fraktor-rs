#![feature(register_tool)]
#![register_tool(ambiguous_suffix)]
#![warn(ambiguous_suffix::ambiguous_suffix)]

use std::marker::PhantomData;

type CallbackMapper = fn(request_manager: usize);

const DEFAULT_ENGINE: usize = 0;
static SESSION_SERVICE: usize = 1;

macro_rules! helper_util {
  () => {};
}

struct InternalManager<TaskService, const DEFAULT_RUNTIME: usize> {
  worker_service: usize,
  _marker: PhantomData<TaskService>,
  _buffer: [u8; DEFAULT_RUNTIME],
}

enum RetryPolicy {
  NetworkEngine,
  Named { request_manager: usize },
}

trait RoutePlanner {
  type ResponseService;
  const FALLBACK_RUNTIME: usize;
  fn select_engine(&self, route_manager: usize);
}

impl RoutePlanner for InternalManager<(), 1> {
  type ResponseService = usize;
  const FALLBACK_RUNTIME: usize = DEFAULT_ENGINE;

  fn select_engine(&self, route_manager: usize) {
    let selected_service = route_manager;
    let _ = selected_service;
  }
}

impl<T, const N: usize> InternalManager<T, N> {
  const CACHE_SERVICE: usize = DEFAULT_ENGINE;

  fn dispatch_engine(&self, task_manager: usize) {
    let worker_runtime = task_manager;
    let (result_service, _ignored) = (worker_runtime, ());
    let _ = result_service;
  }
}

fn load_facade(handler_runtime: usize) {
  let query_engine = handler_runtime;
  let _ = query_engine;
}

fn main() {
  helper_util!();
}
