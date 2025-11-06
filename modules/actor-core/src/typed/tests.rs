use core::hint::spin_loop;

use crate::{
  NoStdToolbox,
  error::ActorError,
  typed::{
    actor_prim::{TypedActor, TypedActorContextGeneric},
    behavior::BehaviorGeneric,
    system::TypedActorSystemGeneric,
  },
};

#[derive(Clone, Copy)]
enum CounterMessage {
  Increment(i32),
  Get,
}

struct CounterActor {
  total: i32,
}

impl CounterActor {
  const fn new() -> Self {
    Self { total: 0 }
  }
}

impl TypedActor<CounterMessage> for CounterActor {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, CounterMessage>,
    message: &CounterMessage,
  ) -> Result<(), ActorError> {
    match message {
      | CounterMessage::Increment(delta) => {
        self.total += delta;
        Ok(())
      },
      | CounterMessage::Get => {
        ctx.reply(self.total).map_err(|error| ActorError::from_send_error(&error))?;
        Ok(())
      },
    }
  }
}

#[test]
fn typed_actor_system_handles_basic_flow() {
  let behavior = BehaviorGeneric::<CounterMessage, NoStdToolbox>::new(CounterActor::new);
  let system = TypedActorSystemGeneric::<CounterMessage, NoStdToolbox>::new(&behavior).expect("system");
  let counter = system.user_guardian_ref();

  counter.tell(CounterMessage::Increment(2)).expect("tell increment one");
  counter.tell(CounterMessage::Increment(5)).expect("tell increment two");

  let response = counter.ask(CounterMessage::Get).expect("ask get");
  let future = response.future().clone();
  wait_until(|| future.is_ready());
  let reply = future.try_take().expect("reply available");
  let payload = reply.payload().downcast_ref::<i32>().copied().expect("payload downcast");

  assert_eq!(payload, 7);

  system.terminate().expect("terminate");
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}
