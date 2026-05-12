//! Classic FSM runtime for kernel extensions and classic actors.

mod fsm_named_timer;
mod fsm_reason;
mod fsm_state_timeout;
mod fsm_timer_fired;
mod fsm_transition;
mod logging_fsm;
mod machine;

#[cfg(test)]
#[path = "fsm_test.rs"]
mod tests;

pub use fsm_reason::FsmReason;
pub use fsm_state_timeout::FsmStateTimeout;
pub use fsm_timer_fired::FsmTimerFired;
pub use fsm_transition::FsmTransition;
pub use logging_fsm::LoggingFsm;
pub use machine::Fsm;
