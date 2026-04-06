use crate::core::kernel::event::logging::{LogLevel, NoLogging};

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
