//! Crate-private contract aliases backing the public DSL surface.
//!
//! Public `core::dsl::*` types should depend on this module instead of
//! referencing `core::stage::*` directly, so the stage package stays internal.

pub(crate) type Flow<In, Out, Mat> = crate::core::dsl::Flow<In, Out, Mat>;
pub(crate) type Sink<In, Mat> = crate::core::dsl::Sink<In, Mat>;
pub(crate) type Source<Out, Mat> = crate::core::dsl::Source<Out, Mat>;
pub(crate) type TailSource<Out> = crate::core::dsl::TailSource<Out>;
