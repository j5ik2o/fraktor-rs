//! Message invocation primitives and middleware pipeline.

mod invoker_trait;
mod middleware;
mod pipeline;

pub use invoker_trait::MessageInvoker;
pub use middleware::MessageInvokerMiddleware;
pub use pipeline::MessageInvokerPipeline;

#[cfg(test)]
mod tests;
