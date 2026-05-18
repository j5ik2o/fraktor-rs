//! Classic deploy configuration surface.

mod deployer;
mod descriptor;
mod remote_scope;
mod scope;

pub use deployer::Deployer;
pub use descriptor::Deploy;
pub use remote_scope::RemoteScope;
pub use scope::Scope;
