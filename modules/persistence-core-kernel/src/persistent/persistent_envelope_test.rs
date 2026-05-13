use core::any::Any;

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  journal::EventAdapters,
  persistent::{PersistentEnvelope, PersistentRepr},
};

struct Counter;

#[test]
fn persistent_envelope_into_repr() {
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(5_i32);
  let envelope = PersistentEnvelope::new(payload.clone(), 3, Box::new(|_actor: &mut Counter, _| {}), true, None);

  assert!(envelope.is_stashing());

  let repr: PersistentRepr = envelope.into_persistent_repr("pid-1", EventAdapters::new());
  assert_eq!(repr.sequence_nr(), 3);
  assert_eq!(repr.downcast_ref::<i32>(), Some(&5));
  assert_eq!(repr.sender(), None);
}
