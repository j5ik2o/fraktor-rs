//! Builder for configuring and constructing scatter-gather-first-completed pool routers.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use core::time::Duration;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::{
  kernel::event::logging::LogLevel,
  typed::{TypedActorRef, behavior::Behavior, dsl::Behaviors, message_and_signals::BehaviorSignal, props::TypedProps},
};

/// Shared closure that rewrites an incoming message with a new reply target.
type CreateRequestFn<M, R> = ArcShared<dyn Fn(&M, TypedActorRef<R>) -> M + Send + Sync>;

/// Shared closure that extracts the original reply target from an incoming message.
type ExtractReplyToFn<M, R> = ArcShared<dyn Fn(&M) -> Option<TypedActorRef<R>> + Send + Sync>;

/// Configures and builds a scatter-gather-first-completed pool router behavior.
///
/// The resulting behavior spawns `pool_size` child actors. For each incoming
/// request that carries a reply target, the router creates a short-lived
/// coordinator actor and sends the request to **all** routees pointing at that
/// coordinator. The first reply received by the coordinator is forwarded to the
/// original sender. If no reply arrives within `within`, the pre-configured
/// `timeout_reply` is sent instead.
///
/// Messages without a reply target (where `extract_reply_to` returns `None`)
/// are broadcast to all routees via fire-and-forget.
pub struct ScatterGatherFirstCompletedRouterBuilder<M, R>
where
  M: Send + Sync + Clone + 'static,
  R: Send + Sync + Clone + 'static, {
  pool_size: usize,
  behavior_factory: ArcShared<dyn Fn() -> Behavior<M> + Send + Sync>,
  within: Duration,
  create_request: CreateRequestFn<M, R>,
  extract_reply_to: ExtractReplyToFn<M, R>,
  timeout_reply: ArcShared<R>,
  pub(crate) force_routee_spawn_failure: bool,
  pub(crate) force_coord_spawn_failure: bool,
}

impl<M, R> ScatterGatherFirstCompletedRouterBuilder<M, R>
where
  M: Send + Sync + Clone + 'static,
  R: Send + Sync + Clone + 'static,
{
  /// Creates a new scatter-gather-first-completed pool router builder.
  ///
  /// # Arguments
  ///
  /// * `pool_size` - Number of routee child actors to spawn.
  /// * `behavior_factory` - Factory for creating routee behaviors.
  /// * `within` - Maximum duration to wait for the first reply.
  /// * `create_request` - Rewrites the incoming message with a new reply target for each routee.
  /// * `extract_reply_to` - Extracts the original reply target from the incoming message. Returns
  ///   `None` for fire-and-forget messages.
  /// * `timeout_reply` - Reply sent to the original sender when no routee responds within the
  ///   timeout.
  ///
  /// # Panics
  ///
  /// Panics if `pool_size` is zero.
  pub(crate) fn new<BF, CF, EF>(
    pool_size: usize,
    behavior_factory: BF,
    within: Duration,
    create_request: CF,
    extract_reply_to: EF,
    timeout_reply: R,
  ) -> Self
  where
    BF: Fn() -> Behavior<M> + Send + Sync + 'static,
    CF: Fn(&M, TypedActorRef<R>) -> M + Send + Sync + 'static,
    EF: Fn(&M) -> Option<TypedActorRef<R>> + Send + Sync + 'static, {
    assert!(pool_size > 0, "pool size must be positive");
    Self {
      pool_size,
      behavior_factory: ArcShared::new(behavior_factory),
      within,
      create_request: ArcShared::new(create_request),
      extract_reply_to: ArcShared::new(extract_reply_to),
      timeout_reply: ArcShared::new(timeout_reply),
      force_routee_spawn_failure: false,
      force_coord_spawn_failure: false,
    }
  }

  /// Builds the scatter-gather pool router as a [`Behavior`].
  #[must_use]
  pub fn build(self) -> Behavior<M> {
    let pool_size = self.pool_size;
    let behavior_factory = self.behavior_factory;
    let within = self.within;
    let create_request = self.create_request;
    let extract_reply_to = self.extract_reply_to;
    let timeout_reply = self.timeout_reply;
    let force_routee_spawn_failure = self.force_routee_spawn_failure;
    let force_coord_spawn_failure = self.force_coord_spawn_failure;

    Behaviors::setup(move |ctx| {
      let bf = behavior_factory.clone();
      let props = TypedProps::<M>::from_behavior_factory(move || {
        let factory: &(dyn Fn() -> Behavior<M> + Send + Sync) = &*bf;
        factory()
      });
      let props = if force_routee_spawn_failure {
        props.with_dispatcher_from_config("__test_force_spawn_failure__")
      } else {
        props
      };

      let mut routee_vec: Vec<TypedActorRef<M>> = Vec::with_capacity(pool_size);
      for _ in 0..pool_size {
        match ctx.spawn_child_watched(&props) {
          | Ok(child) => routee_vec.push(child.into_actor_ref()),
          | Err(e) => {
            let msg = alloc::format!("scatter-gather router failed to spawn child: {:?}", e);
            ctx.system().emit_log(LogLevel::Warn, msg, Some(ctx.pid()), None);
            break;
          },
        }
      }

      // routee が1体も起動できなかった場合はルーターを停止する
      if routee_vec.is_empty() {
        ctx.system().emit_log(LogLevel::Error, "scatter-gather router has no routees, stopping", Some(ctx.pid()), None);
        return Behaviors::stopped();
      }

      let routees = ArcShared::new(RuntimeMutex::new(routee_vec));
      let routees_for_msg = routees.clone();
      let routees_for_sig = routees;
      let create_request = create_request.clone();
      let extract_reply_to = extract_reply_to.clone();
      let timeout_reply = timeout_reply.clone();

      let force_coord = force_coord_spawn_failure;

      Behaviors::receive_message(move |ctx, message: &M| {
        let routee_snapshot: Vec<TypedActorRef<M>> = {
          let guard = routees_for_msg.lock();
          if guard.is_empty() {
            return Ok(Behaviors::same());
          }
          guard.to_vec()
        };

        if let Some(original_reply_to) = (extract_reply_to)(message) {
          spawn_gather_coordinator(
            ctx,
            &routee_snapshot,
            message,
            original_reply_to,
            &create_request,
            within,
            &timeout_reply,
            force_coord,
          );
        } else {
          for routee in &routee_snapshot {
            let mut r = routee.clone();
            if let Err(error) = r.try_tell(message.clone()) {
              ctx.system().emit_log(
                LogLevel::Warn,
                alloc::format!("scatter-gather router failed to deliver message to routee: {:?}", error),
                Some(ctx.pid()),
                None,
              );
            }
          }
        }

        Ok(Behaviors::same())
      })
      .receive_signal(move |_ctx, signal| match signal {
        | BehaviorSignal::Terminated(pid) => {
          let mut guard = routees_for_sig.lock();
          if let Some(pos) = guard.iter().position(|r| r.pid() == *pid) {
            guard.remove(pos);
          }
          if guard.is_empty() {
            return Ok(Behaviors::stopped());
          }
          Ok(Behaviors::same())
        },
        | _ => Ok(Behaviors::same()),
      })
    })
  }
}

/// Spawns a one-shot coordinator that forwards the first reply from any routee.
#[allow(clippy::too_many_arguments)]
fn spawn_gather_coordinator<'a, M, R>(
  ctx: &mut crate::core::typed::actor::TypedActorContext<'a, M>,
  routees: &[TypedActorRef<M>],
  message: &M,
  reply_to: TypedActorRef<R>,
  create_request: &CreateRequestFn<M, R>,
  within: Duration,
  timeout_reply: &R,
  force_spawn_failure: bool,
) where
  M: Send + Sync + Clone + 'static,
  R: Send + Sync + Clone + 'static, {
  // spawn 失敗時に timeout_reply を返すため、事前に控えておく
  let mut fallback_reply_to = reply_to.clone();
  let fallback_timeout_reply = timeout_reply.clone();

  let coord_props = TypedProps::<R>::from_behavior_factory(move || -> Behavior<R> {
    let rt = reply_to.clone();
    Behaviors::receive_message(move |_ctx, msg: &R| {
      let mut reply_to = rt.clone();
      if let Err(_error) = reply_to.try_tell(msg.clone()) {}
      Ok(Behaviors::stopped())
    })
  });
  let coord_props = if force_spawn_failure {
    coord_props.with_dispatcher_from_config("__test_force_spawn_failure__")
  } else {
    coord_props
  };

  match ctx.spawn_child(&coord_props) {
    | Ok(coord_child) => {
      let coord_ref = coord_child.actor_ref();
      for routee in routees {
        let mut r = routee.clone();
        if let Err(error) = r.try_tell((create_request)(message, coord_ref.clone())) {
          ctx.system().emit_log(
            LogLevel::Warn,
            alloc::format!("scatter-gather coordinator failed to deliver request: {:?}", error),
            Some(ctx.pid()),
            None,
          );
        }
      }
      if let Err(e) = ctx.schedule_once(within, coord_ref, timeout_reply.clone()) {
        ctx.system().emit_log(
          LogLevel::Warn,
          alloc::format!("scatter-gather coordinator failed to schedule timeout: {:?}", e),
          Some(ctx.pid()),
          None,
        );
      }
    },
    | Err(e) => {
      let msg = alloc::format!("scatter-gather coordinator spawn failed: {:?}", e);
      ctx.system().emit_log(LogLevel::Warn, msg, Some(ctx.pid()), None);
      // caller が無応答にならないよう timeout_reply を即時返却する
      if let Err(_error) = fallback_reply_to.try_tell(fallback_timeout_reply) {}
    },
  }
}
