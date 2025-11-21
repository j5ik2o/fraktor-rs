use crate::core::schema_negotiator::SchemaNegotiator;

#[test]
fn picks_highest_common_version() {
  let negotiator = SchemaNegotiator::new(vec![1, 2, 3]);
  let result = negotiator.negotiate(&[2, 3]);
  assert_eq!(result, Some(3));
}

#[test]
fn returns_none_when_incompatible() {
  let negotiator = SchemaNegotiator::new(vec![1, 2]);
  assert_eq!(negotiator.negotiate(&[3, 4]), None);
}
