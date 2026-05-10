#[cfg(test)]
mod tests;

/// Linear chain macro for `GraphDslBuilder`.
///
/// Connects a source through zero or more flows to a sink in one expression.
/// Must be called in a context that supports the `?` operator.
///
/// # Examples
///
/// ```text
/// graph_chain!(builder; source => flow1 => flow2 => sink);
/// ```
///
/// Expands to:
///
/// ```text
/// let __chain_out = builder.add_source(source)?;
/// let __chain_out = builder.wire_via(&__chain_out, flow1)?;
/// let __chain_out = builder.wire_via(&__chain_out, flow2)?;
/// builder.wire_to(&__chain_out, sink)?;
/// ```
#[macro_export]
macro_rules! graph_chain {
  // Internal: terminal — only sink remains.
  (@step $builder:expr, $prev:ident; $sink:expr) => {
    $builder.wire_to(&$prev, $sink)?
  };
  // Internal: recursive — consume one flow and continue.
  (@step $builder:expr, $prev:ident; $flow:expr => $($rest:tt)+) => {{
    let $prev = $builder.wire_via(&$prev, $flow)?;
    graph_chain!(@step $builder, $prev; $($rest)+)
  }};
  // Entry point: extract source and delegate to @step.
  ($builder:expr; $src:expr => $($rest:tt)+) => {{
    let __chain_out = $builder.add_source($src)?;
    graph_chain!(@step $builder, __chain_out; $($rest)+)
  }};
}
