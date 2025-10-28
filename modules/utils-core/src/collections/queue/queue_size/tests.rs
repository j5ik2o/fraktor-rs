use super::QueueSize;

#[test]
fn queue_size_helpers_work_as_expected() {
  let zero = QueueSize::limited(0);
  let limitless = QueueSize::limitless();

  assert!(!zero.is_limitless());
  assert_eq!(zero.to_usize(), 0);

  assert!(limitless.is_limitless());
  assert_eq!(limitless.to_usize(), usize::MAX);

  match limitless {
    | QueueSize::Limitless => {},
    | _ => panic!("expected limitless variant"),
  }

  match zero {
    | QueueSize::Limited(value) => assert_eq!(value, 0),
    | _ => panic!("expected limited variant"),
  }
}
