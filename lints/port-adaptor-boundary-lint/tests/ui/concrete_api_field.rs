#![feature(register_tool)]
#![allow(dead_code)]
#![register_tool(port_adaptor_boundary)]
#![warn(port_adaptor_boundary)]

extern crate self as fraktor_cluster_core_kernel_rs;

pub mod extension {
  pub struct ClusterApi;
}

#[path = "auxiliary/modules/cluster-adaptor-std/src/concrete_api_field.rs"]
mod concrete_api_field;

fn main() {}
