use super::DownstreamCancellationControlPlane;
use crate::core::StreamError;

#[test]
fn replace_routes_resets_recorded_cancellation_failure() {
  let mut control_plane = DownstreamCancellationControlPlane::new(Vec::new());
  control_plane.first_failure = Some(StreamError::Failed);

  control_plane.replace_routes(Vec::new());
  let result = control_plane.propagate(|_| Ok(()));

  assert_eq!(result, Ok(()));
}
