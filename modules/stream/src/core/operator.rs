//! Operator compatibility catalog.

// Re-import for children
use super::StreamDslError;

mod default_operator_catalog;
mod operator_catalog;
mod operator_contract;
mod operator_coverage;
mod operator_key;

pub use default_operator_catalog::DefaultOperatorCatalog;
pub use operator_catalog::OperatorCatalog;
pub use operator_contract::OperatorContract;
pub use operator_coverage::OperatorCoverage;
pub use operator_key::OperatorKey;
