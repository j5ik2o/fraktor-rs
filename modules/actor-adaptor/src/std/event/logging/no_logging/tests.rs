use fraktor_actor_rs::core::kernel::event::logging::LogLevel;

use super::NoLogging;

#[test]
fn no_logging_accepts_all_log_calls() {
  let logging = NoLogging;

  logging.trace("trace");
  logging.debug("debug");
  logging.info("info");
  logging.warn("warn");
  logging.error("error");
  logging.log(LogLevel::Warn, "generic");
}
