use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{persistent_envelope::PersistentEnvelope, persistent_repr::PersistentRepr};

struct Counter;

#[test]
fn persistent_envelope_into_repr() {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(5_i32);
  let envelope = PersistentEnvelope::new(payload.clone(), 3, Box::new(|_actor: &mut Counter, _| {}), true);

  assert!(envelope.is_stashing());

  let repr: PersistentRepr = envelope.into_persistent_repr("pid-1");
  assert_eq!(repr.sequence_nr(), 3);
  assert_eq!(repr.downcast_ref::<i32>(), Some(&5));
}
