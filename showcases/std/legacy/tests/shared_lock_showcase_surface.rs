const SHARED_LOCK_SHOWCASES: &[&str] = &[
  "../classic_timers/main.rs",
  "../classic_logging/main.rs",
  "../routing/main.rs",
  "../typed_event_stream/main.rs",
  "../typed_receptionist_router/main.rs",
  "../timers/main.rs",
];

fn shared_lock_showcase_sources() -> impl Iterator<Item = &'static str> {
  SHARED_LOCK_SHOWCASES.iter().copied().map(|path| match path {
    | "../classic_timers/main.rs" => include_str!("../classic_timers/main.rs"),
    | "../classic_logging/main.rs" => include_str!("../classic_logging/main.rs"),
    | "../routing/main.rs" => include_str!("../routing/main.rs"),
    | "../typed_event_stream/main.rs" => include_str!("../typed_event_stream/main.rs"),
    | "../typed_receptionist_router/main.rs" => include_str!("../typed_receptionist_router/main.rs"),
    | "../timers/main.rs" => include_str!("../timers/main.rs"),
    | _ => unreachable!("unknown showcase path"),
  })
}

const SHARED_LOCK_MAIN_SHOWCASES: &[&str] = &[
  "../classic_timers/main.rs",
  "../classic_logging/main.rs",
  "../routing/main.rs",
  "../typed_event_stream/main.rs",
  "../typed_receptionist_router/main.rs",
  "../timers/main.rs",
];

fn shared_lock_main_showcase_sources() -> impl Iterator<Item = &'static str> {
  SHARED_LOCK_MAIN_SHOWCASES.iter().copied().map(|path| match path {
    | "../classic_timers/main.rs" => include_str!("../classic_timers/main.rs"),
    | "../classic_logging/main.rs" => include_str!("../classic_logging/main.rs"),
    | "../routing/main.rs" => include_str!("../routing/main.rs"),
    | "../typed_event_stream/main.rs" => include_str!("../typed_event_stream/main.rs"),
    | "../typed_receptionist_router/main.rs" => include_str!("../typed_receptionist_router/main.rs"),
    | "../timers/main.rs" => include_str!("../timers/main.rs"),
    | _ => unreachable!("unknown showcase path"),
  })
}

#[test]
fn shared_lock_showcases_do_not_reference_removed_no_std_mutex_alias() {
  for source in shared_lock_showcase_sources() {
    assert!(!source.contains("NoStdMutex"), "shared lock showcase must not reference removed NoStdMutex alias");
  }
}

#[test]
fn shared_lock_showcases_do_not_reference_removed_runtime_lock_aliases() {
  for source in shared_lock_showcase_sources() {
    assert!(!source.contains("RuntimeMutex"), "shared lock showcase must not reference removed RuntimeMutex alias");
    assert!(!source.contains("RuntimeRwLock"), "shared lock showcase must not reference removed RuntimeRwLock alias");
  }
}

#[test]
fn shared_lock_main_showcases_do_not_use_removed_shared_lock_guard_api() {
  for source in shared_lock_main_showcase_sources() {
    assert!(!source.contains(".lock()"), "shared lock showcase must use closure-based access instead of .lock()");
  }
}
