use crate::core::grain_key::GrainKey;

#[test]
fn value_is_preserved() {
  let key = GrainKey::new("user:1".to_string());
  assert_eq!(key.value(), "user:1");
}
