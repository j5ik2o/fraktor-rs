//! Work-pulling producer controller for reliable delivery across multiple workers.

use super::internal::WorkPullingProducerController as InternalWorkPullingProducerController;

/// Work-pulling producer controller for reliable delivery across multiple workers.
pub type WorkPullingProducerController = InternalWorkPullingProducerController;
