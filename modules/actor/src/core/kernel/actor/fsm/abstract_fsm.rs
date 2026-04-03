//! Classic AbstractFSM alias over the kernel FSM runtime.

use super::Fsm;

/// Classic `AbstractFSM` compatibility alias.
pub type AbstractFsm<State, Data> = Fsm<State, Data>;
