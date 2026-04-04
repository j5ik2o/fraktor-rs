#[test]
fn std_pattern_reexports_core_helpers() {
  let _ = crate::std::pattern::ask_with_timeout;
  let _ = crate::std::pattern::graceful_stop;
  let _ = crate::std::pattern::graceful_stop_with_message;
  let mut delay_provider = fraktor_utils_rs::core::timing::delay::ManualDelayProvider::new();
  let _future = crate::std::pattern::retry(
    1,
    &mut delay_provider,
    |_| core::time::Duration::ZERO,
    || core::future::ready(Ok::<(), ()>(())),
  );
}

#[test]
fn std_pattern_provides_circuit_breaker_types() {
  // CircuitBreaker factory function available
  let _cb = crate::std::pattern::circuit_breaker(1, core::time::Duration::from_millis(100));
  // CircuitBreakerShared factory function available
  let _cbs = crate::std::pattern::circuit_breaker_shared(1, core::time::Duration::from_millis(100));
  // State enum from core::pattern
  let _state = fraktor_actor_rs::core::kernel::pattern::CircuitBreakerState::Closed;
}
