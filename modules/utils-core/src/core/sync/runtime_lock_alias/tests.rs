use super::NoStdMutex;

#[test]
fn no_std_mutex_alias_uses_default_spin_driver() {
  let mutex: NoStdMutex<_> = NoStdMutex::new(3_u32);
  assert_eq!(*mutex.lock(), 3);
}
