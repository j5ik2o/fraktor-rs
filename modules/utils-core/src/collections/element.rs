use core::fmt::Debug;

use crate::SharedBound;

/// Fundamental constraints for elements that can be stored in collections such as queues and
/// stacks.
///
///
///
///
/// On targets that provide atomic pointer support we demand `Send + Sync` so that elements can be
/// safely shared across threads. On single-threaded targets (e.g. RP2040) we only require `Debug`
/// and `'static`, allowing `Rc`-based implementations to operate without unnecessary bounds.
pub trait Element: Debug + SharedBound + 'static {}

impl<T> Element for T where T: Debug + SharedBound + 'static {}
