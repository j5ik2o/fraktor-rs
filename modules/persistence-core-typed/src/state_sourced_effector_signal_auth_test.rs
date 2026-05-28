use crate::state_sourced_effector_signal_auth::StateSourcedEffectorSignalAuth;

#[test]
fn auth_marker_is_constructible_inside_crate() {
  let auth = StateSourcedEffectorSignalAuth::new();

  assert_eq!(auth, StateSourcedEffectorSignalAuth::new());
}
