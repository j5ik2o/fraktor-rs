//! Reliable delivery between producers and consumers.

/// Consumer controller actor for reliable delivery.
mod consumer_controller;
/// Commands accepted by the consumer controller.
mod consumer_controller_command;
/// Confirmation message from consumer to consumer controller.
mod consumer_controller_confirmed;
/// Delivery wrapper sent to the consumer.
mod consumer_controller_delivery;
/// Settings for the consumer controller.
mod consumer_controller_settings;
/// Producer controller actor for reliable delivery.
mod producer_controller;
/// Commands accepted by the producer controller.
mod producer_controller_command;
/// Demand signal from producer controller to the producer.
mod producer_controller_request_next;
/// Settings for the producer controller.
mod producer_controller_settings;
/// Sequence number type.
mod seq_nr;
/// Wire-protocol message between controllers.
mod sequenced_message;
/// Work-pulling producer controller for multi-worker reliable delivery.
mod work_pulling_producer_controller;
/// Commands accepted by the work-pulling producer controller.
mod work_pulling_producer_controller_command;
/// Demand signal from work-pulling producer controller to the producer.
mod work_pulling_producer_controller_request_next;
/// Settings for the work-pulling producer controller.
mod work_pulling_producer_controller_settings;
/// Statistics about registered workers.
mod worker_stats;

pub use consumer_controller::ConsumerController;
pub use consumer_controller_command::ConsumerControllerCommand;
pub use consumer_controller_confirmed::ConsumerControllerConfirmed;
pub use consumer_controller_delivery::ConsumerControllerDelivery;
pub use consumer_controller_settings::ConsumerControllerSettings;
pub use producer_controller::ProducerController;
pub use producer_controller_command::ProducerControllerCommand;
pub use producer_controller_request_next::ProducerControllerRequestNext;
pub(crate) use producer_controller_settings::ProducerControllerSettings;
pub use seq_nr::SeqNr;
pub use sequenced_message::SequencedMessage;
pub use work_pulling_producer_controller::WorkPullingProducerController;
pub use work_pulling_producer_controller_command::WorkPullingProducerControllerCommand;
pub use work_pulling_producer_controller_request_next::WorkPullingProducerControllerRequestNext;
pub(crate) use work_pulling_producer_controller_settings::WorkPullingProducerControllerSettings;
pub use worker_stats::WorkerStats;

#[cfg(test)]
mod tests;
