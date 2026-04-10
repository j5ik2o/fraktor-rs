use super::StdSyncMutex;

#[test]
fn locks_and_mutates_value() {
  let mutex = StdSyncMutex::new(1_u32);
  *mutex.lock() = 3;
  assert_eq!(*mutex.lock(), 3);
}
