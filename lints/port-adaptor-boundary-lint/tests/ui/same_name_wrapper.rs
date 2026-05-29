#![feature(register_tool)]
#![allow(dead_code)]
#![register_tool(port_adaptor_boundary)]
#![warn(port_adaptor_boundary)]

extern crate self as fraktor_cluster_core_kernel_rs;

pub mod grain {
  pub struct GrainRef;
}

#[path = "auxiliary/modules/cluster-adaptor-std/src/same_name_wrapper.rs"]
mod same_name_wrapper;

fn main() {}
