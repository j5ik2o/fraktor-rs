#![feature(register_tool)]
#![allow(dead_code)]
#![register_tool(port_adaptor_boundary)]
#![warn(port_adaptor_boundary)]

extern crate self as fraktor_cluster_core_kernel_rs;

pub mod extension {
  pub trait ClusterIdentityResolver {}
}

#[path = "auxiliary/modules/cluster-adaptor-std/src/port_trait_ok.rs"]
mod port_trait_ok;

fn main() {}
