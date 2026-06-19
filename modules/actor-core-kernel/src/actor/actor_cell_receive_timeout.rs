//! Receive timeout facet for actor cells.

use crate::actor::{ActorCell, ActorContext, messaging::AnyMessage};

impl ActorCell {
  pub(super) fn reschedule_receive_timeout_after_user_success(ctx: &mut ActorContext<'_>, message: &AnyMessage) {
    // Pekko `dungeon/ReceiveTimeout.scala:40-42`
    // `checkReceiveTimeoutIfNeeded`: marker messages skip timeout refresh.
    if !message.is_not_influence_receive_timeout() {
      ctx.reschedule_receive_timeout();
    }
  }
}
