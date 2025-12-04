//! Message invocation primitives and middleware pipeline.

mod invoker_shared;
mod invoker_trait;
mod middleware;
mod middleware_shared;
mod pipeline;

pub use invoker_shared::MessageInvokerShared;
pub use invoker_trait::MessageInvoker;
pub use middleware::MessageInvokerMiddleware;
pub use pipeline::{MessageInvokerPipeline, MessageInvokerPipelineGeneric};

#[cfg(test)]
mod tests;
