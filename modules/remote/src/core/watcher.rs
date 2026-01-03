//! Remote actor watching infrastructure.

mod command;
mod daemon;

pub(crate) use command::RemoteWatcherCommand;
pub(crate) use daemon::RemoteWatcherDaemon;
