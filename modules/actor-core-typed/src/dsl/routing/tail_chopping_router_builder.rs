//! Builder for configuring and constructing tail-chopping pool routers.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use core::time::Duration;

use fraktor_actor_core_kernel_rs::event::logging::LogLevel;
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};

use crate::{
  TypedActorRef, actor::TypedActorContext, behavior::Behavior, dsl::Behaviors, message_adapter::AdapterError,
  message_and_signals::BehaviorSignal, props::TypedProps,
};

/// Shared closure that rewrites an incoming message with a new reply target.
type CreateRequestFn<M, R> = ArcShared<dyn Fn(&M, TypedActorRef<R>) -> M + Send + Sync>;

/// Shared closure that extracts the original reply target from an incoming message.
type ExtractReplyToFn<M, R> = ArcShared<dyn Fn(&M) -> Option<TypedActorRef<R>> + Send + Sync>;

/// Configures and builds a tail-chopping pool router behavior.
///
/// Tail-chopping reduces tail latency by sending backup requests: the router
/// sends the request to one routee at a time, waiting `interval` between each
/// attempt. The first reply from any routee is forwarded to the original sender
/// and all remaining attempts are abandoned.
///
/// If no routee responds within `within`, the pre-configured `timeout_reply` is
/// sent to the original sender.
pub struct TailChoppingRouterBuilder<M, R>
where
  M: Send + Sync + Clone + 'static,
  R: Send + Sync + Clone + 'static, {
  pool_size:        usize,
  behavior_factory: ArcShared<dyn Fn() -> Behavior<M> + Send + Sync>,
  within:           Duration,
  interval:         Duration,
  create_request:   CreateRequestFn<M, R>,
  extract_reply_to: ExtractReplyToFn<M, R>,
  timeout_reply:    ArcShared<R>,
}

impl<M, R> TailChoppingRouterBuilder<M, R>
where
  M: Send + Sync + Clone + 'static,
  R: Send + Sync + Clone + 'static,
{
  /// Creates a new tail-chopping pool router builder.
  ///
  /// # Arguments
  ///
  /// * `pool_size` - Number of routee child actors to spawn.
  /// * `behavior_factory` - Factory for creating routee behaviors.
  /// * `within` - Maximum duration to wait for any reply.
  /// * `interval` - Duration between successive send attempts to the next routee.
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
    interval: Duration,
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
      interval,
      create_request: ArcShared::new(create_request),
      extract_reply_to: ArcShared::new(extract_reply_to),
      timeout_reply: ArcShared::new(timeout_reply),
    }
  }

  /// Builds the tail-chopping pool router as a [`Behavior`].
  #[must_use]
  pub fn build(self) -> Behavior<M> {
    let pool_size = self.pool_size;
    let behavior_factory = self.behavior_factory;
    let within = self.within;
    let interval = self.interval;
    let create_request = self.create_request;
    let extract_reply_to = self.extract_reply_to;
    let timeout_reply = self.timeout_reply;

    Behaviors::setup(move |ctx| {
      let bf = behavior_factory.clone();
      let props = TypedProps::<M>::from_behavior_factory(move || {
        let factory: &(dyn Fn() -> Behavior<M> + Send + Sync) = &*bf;
        factory()
      });

      let mut routee_vec: Vec<TypedActorRef<M>> = Vec::with_capacity(pool_size);
      for _ in 0..pool_size {
        match ctx.spawn_child_watched(&props) {
          | Ok(child) => routee_vec.push(child.into_actor_ref()),
          | Err(e) => {
            let msg = alloc::format!("tail-chopping router failed to spawn child: {:?}", e);
            ctx.system().emit_log(LogLevel::Warn, msg, Some(ctx.pid()), None);
            break;
          },
        }
      }

      // routee が1体も起動できなかった場合はルーターを停止する
      if routee_vec.is_empty() {
        ctx.system().emit_log(LogLevel::Error, "tail-chopping router has no routees, stopping", Some(ctx.pid()), None);
        return Behaviors::stopped();
      }

      let routees = SharedLock::new_with_driver::<DefaultMutex<_>>(routee_vec);
      let routees_for_msg = routees.clone();
      let routees_for_sig = routees;
      let create_request = create_request.clone();
      let extract_reply_to = extract_reply_to.clone();
      let timeout_reply = timeout_reply.clone();

      Behaviors::receive_message(move |ctx, message: &M| {
        let routee_snapshot = routees_for_msg.with_lock(|guard| guard.clone());
        if routee_snapshot.is_empty() {
          return Ok(Behaviors::same());
        }

        if let Some(original_reply_to) = (extract_reply_to)(message) {
          spawn_chop_coordinator(ctx, ChopCoordinatorParams {
            routees: routee_snapshot,
            message: message.clone(),
            reply_to: original_reply_to,
            create_request: create_request.clone(),
            within,
            interval,
            timeout_reply: (*timeout_reply).clone(),
          });
        } else {
          // reply_to なし（fire-and-forget）— 最初の1台のみに送信する（broadcast しない）
          if let Some(first) = routee_snapshot.first() {
            let mut r = first.clone();
            if let Err(error) = r.try_tell(message.clone()) {
              ctx.system().emit_log(
                LogLevel::Warn,
                alloc::format!("tail-chopping router failed to deliver message to routee: {:?}", error),
                Some(ctx.pid()),
                None,
              );
            }
          }
        }

        Ok(Behaviors::same())
      })
      .receive_signal(move |_ctx, signal| match signal {
        | BehaviorSignal::Terminated(terminated) => {
          let pid = terminated.pid();
          let is_empty = routees_for_sig.with_lock(|guard| {
            if let Some(pos) = guard.iter().position(|r| r.pid() == pid) {
              guard.remove(pos);
            }
            guard.is_empty()
          });
          if is_empty {
            return Ok(Behaviors::stopped());
          }
          Ok(Behaviors::same())
        },
        | _ => Ok(Behaviors::same()),
      })
    })
  }
}

// ---------------------------------------------------------------------------
// 内部コーディネーター
// ---------------------------------------------------------------------------

/// Internal command type for the tail-chopping coordinator.
#[derive(Clone)]
enum ChopCommand<R>
where
  R: Clone, {
  /// A reply was received from a routee (via message adapter).
  Reply(R),
  /// Timer fired: send the request to the next routee.
  SendNext,
  /// Timer fired: overall timeout expired.
  FinalTimeout,
}

/// Parameters for spawning a tail-chopping coordinator.
struct ChopCoordinatorParams<M, R>
where
  M: Send + Sync + Clone + 'static,
  R: Send + Sync + Clone + 'static, {
  routees:        Vec<TypedActorRef<M>>,
  message:        M,
  reply_to:       TypedActorRef<R>,
  create_request: CreateRequestFn<M, R>,
  within:         Duration,
  interval:       Duration,
  timeout_reply:  R,
}

/// Spawns a tail-chopping coordinator child actor.
///
/// On spawn failure the `timeout_reply` is sent back to the caller so that the
/// ask side does not hang indefinitely.
fn spawn_chop_coordinator<'a, M, R>(ctx: &mut TypedActorContext<'a, M>, params: ChopCoordinatorParams<M, R>)
where
  M: Send + Sync + Clone + 'static,
  R: Send + Sync + Clone + 'static, {
  // spawn 失敗時に timeout_reply を返すため、事前に控えておく
  let mut fallback_reply_to = params.reply_to.clone();
  let fallback_timeout_reply = params.timeout_reply.clone();
  let coord_props = chop_coordinator_props(params);

  match ctx.spawn_child(&coord_props) {
    | Ok(_) => {},
    | Err(e) => {
      let msg = alloc::format!("tail-chopping coordinator spawn failed: {:?}", e);
      ctx.system().emit_log(LogLevel::Warn, msg, Some(ctx.pid()), None);
      // caller が無応答にならないよう timeout_reply を即時返却する。
      if let Err(_error) = fallback_reply_to.try_tell(fallback_timeout_reply) {}
    },
  }
}

/// Creates typed props for the tail-chopping coordinator behavior.
fn chop_coordinator_props<M, R>(params: ChopCoordinatorParams<M, R>) -> TypedProps<ChopCommand<R>>
where
  M: Send + Sync + Clone + 'static,
  R: Send + Sync + Clone + 'static, {
  let routees = ArcShared::new(params.routees);
  let message = ArcShared::new(params.message);
  let create_request = params.create_request;
  let reply_to = ArcShared::new(params.reply_to);
  let timeout_reply = ArcShared::new(params.timeout_reply);
  let within = params.within;
  let interval = params.interval;

  TypedProps::<ChopCommand<R>>::from_behavior_factory(move || -> Behavior<ChopCommand<R>> {
    let routees = routees.clone();
    let message = message.clone();
    let create_request = create_request.clone();
    let reply_to = reply_to.clone();
    let timeout_reply = timeout_reply.clone();

    Behaviors::setup(move |ctx| {
      // メッセージアダプターを作成: routee からの R を ChopCommand::Reply(R) として受け取る
      let Ok(adapter_ref) =
        ctx.message_adapter::<R, _>(|r: R| -> Result<ChopCommand<R>, AdapterError> { Ok(ChopCommand::Reply(r)) })
      else {
        return Behaviors::stopped();
      };

      // 最初の routee へ即時送信する
      if let Some(first) = routees.first() {
        let mut r = first.clone();
        if let Err(error) = r.try_tell((create_request)(&message, adapter_ref.clone())) {
          ctx.system().emit_log(
            LogLevel::Warn,
            alloc::format!("tail-chopping coordinator failed to deliver initial request: {:?}", error),
            Some(ctx.pid()),
            None,
          );
        }
      }

      // 次送信のインターバルタイマーを設定する（routee が2体以上の場合のみ）
      if routees.len() > 1
        && let Err(e) = ctx.schedule_once(interval, ctx.self_ref(), ChopCommand::<R>::SendNext)
      {
        ctx.system().emit_log(
          LogLevel::Warn,
          alloc::format!("tail-chopping coordinator failed to schedule next send: {:?}", e),
          Some(ctx.pid()),
          None,
        );
      }
      // 全体タイムアウトを設定する
      if let Err(e) = ctx.schedule_once(within, ctx.self_ref(), ChopCommand::<R>::FinalTimeout) {
        ctx.system().emit_log(
          LogLevel::Warn,
          alloc::format!("tail-chopping coordinator failed to schedule final timeout: {:?}", e),
          Some(ctx.pid()),
          None,
        );
      }

      let current_index = SharedLock::new_with_driver::<DefaultMutex<_>>(1_usize);
      let routees = routees.clone();
      let message = message.clone();
      let create_request = create_request.clone();
      let reply_to = reply_to.clone();
      let timeout_reply = timeout_reply.clone();

      Behaviors::receive_message(move |ctx, cmd: &ChopCommand<R>| match cmd {
        | ChopCommand::Reply(r) => {
          let mut target = (*reply_to).clone();
          if let Err(_error) = target.try_tell(r.clone()) {}
          Ok(Behaviors::stopped())
        },
        | ChopCommand::SendNext => {
          current_index.with_lock(|idx| {
            if *idx < routees.len() {
              let mut r = routees[*idx].clone();
              if let Err(error) = r.try_tell((create_request)(&message, adapter_ref.clone())) {
                ctx.system().emit_log(
                  LogLevel::Warn,
                  alloc::format!("tail-chopping coordinator failed to deliver request: {:?}", error),
                  Some(ctx.pid()),
                  None,
                );
              }
              *idx += 1;
              if *idx < routees.len()
                && let Err(e) = ctx.schedule_once(interval, ctx.self_ref(), ChopCommand::<R>::SendNext)
              {
                ctx.system().emit_log(
                  LogLevel::Warn,
                  alloc::format!("tail-chopping coordinator failed to schedule next send: {:?}", e),
                  Some(ctx.pid()),
                  None,
                );
              }
            }
          });
          Ok(Behaviors::same())
        },
        | ChopCommand::FinalTimeout => {
          let mut target = (*reply_to).clone();
          if let Err(_error) = target.try_tell((*timeout_reply).clone()) {}
          Ok(Behaviors::stopped())
        },
      })
    })
  })
}
