use std::fmt;

fn foo() {}

#[path = "auxiliary/helpers_inline.rs"]
mod helpers;

use std::str::FromStr;

#[cfg(test)]
mod tests;

fn main() {}
