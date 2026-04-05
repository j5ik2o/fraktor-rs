#[test]
fn std_pattern_provides_only_std_specific_circuit_breaker_api() {
  // CircuitBreaker factory function available
  let _cb = crate::std::pattern::circuit_breaker(1, core::time::Duration::from_millis(100));
  // CircuitBreakerShared factory function available
  let _cbs = crate::std::pattern::circuit_breaker_shared(1, core::time::Duration::from_millis(100));
  let _clock = core::marker::PhantomData::<crate::std::pattern::StdClock>;
  let _circuit_breaker = core::marker::PhantomData::<crate::std::pattern::CircuitBreaker>;
  let _circuit_breaker_shared = core::marker::PhantomData::<crate::std::pattern::CircuitBreakerShared>;
  // State enum from core::pattern
  let _state = fraktor_actor_rs::core::kernel::pattern::CircuitBreakerState::Closed;
}
