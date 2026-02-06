#![feature(register_tool)]
#![register_tool(ambiguous_suffix)]
#![warn(ambiguous_suffix::ambiguous_suffix)]

pub struct NormalRegistry;

#[allow(ambiguous_suffix)]
#[allow(ambiguous_suffix::ambiguous_suffix)]
pub struct ExternalApiManager;

fn main() {}
