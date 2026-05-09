use alloc::{borrow::Cow, string::String};

use fraktor_actor_core_kernel_rs::actor::{error::SendError, messaging::AnyMessage};

use crate::{
  r#impl::{FramingErrorKind, StreamError},
  stage::{CancellationCause, CancellationKind},
};

// --- StreamDetached variant ---

#[test]
fn stream_detached_is_constructible() {
  // Given/When: constructing the StreamDetached variant
  let error = StreamError::StreamDetached;

  // Then: it matches the expected variant
  assert!(matches!(error, StreamError::StreamDetached));
}

#[test]
fn stream_detached_display_contains_detached_message() {
  // Given: a StreamDetached error
  let error = StreamError::StreamDetached;

  // When: formatting with Display
  let message = alloc::format!("{error}");

  // Then: the message describes the detached state
  assert!(message.contains("detached"), "expected 'detached' in message: {message}");
}

#[test]
fn stream_detached_is_distinct_from_never_materialized() {
  // Given: both error variants
  let detached = StreamError::StreamDetached;
  let never_mat = StreamError::NeverMaterialized;

  // Then: they are not equal
  assert_ne!(detached, never_mat);
}

#[test]
fn stream_detached_clone_preserves_variant() {
  // Given: a StreamDetached error
  let original = StreamError::StreamDetached;

  // When: cloning
  let cloned = original.clone();

  // Then: clone equals original
  assert_eq!(original, cloned);
}

#[test]
fn stream_error_from_send_error_preserves_send_context() {
  let error = StreamError::from_send_error(&SendError::closed(AnyMessage::new("payload")));

  assert!(error.is_source_type::<SendError>());
  assert!(matches!(error, StreamError::FailedWithContext { .. }));
  assert!(alloc::format!("{error}").contains("send failed"));
}

#[test]
fn stream_error_from_send_error_maps_backpressure_to_would_block() {
  let error = StreamError::from_send_error(&SendError::full(AnyMessage::new("payload")));

  assert_eq!(error, StreamError::WouldBlock);
}

#[test]
fn materialized_resource_rollback_error_exposes_primary_and_cleanup_failures() {
  let error =
    StreamError::materialized_resource_rollback_failed(StreamError::MaterializerNotStarted, StreamError::Failed);

  assert_eq!(error.materialization_primary_failure(), Some(&StreamError::MaterializerNotStarted));
  assert_eq!(error.materialization_cleanup_failure(), Some(&StreamError::Failed));
}

#[test]
fn non_rollback_error_has_no_materialization_failure_parts() {
  let error = StreamError::Failed;

  assert!(error.materialization_primary_failure().is_none());
  assert!(error.materialization_cleanup_failure().is_none());
}

#[test]
fn materialized_resource_rollback_display_includes_both_failures() {
  let error =
    StreamError::materialized_resource_rollback_failed(StreamError::MaterializerNotStarted, StreamError::Failed);
  let rendered = alloc::format!("{error}");

  assert!(rendered.contains("materialization failed"), "unexpected rendering: {rendered}");
  assert!(rendered.contains("materializer not started"), "primary failure must be rendered: {rendered}");
  assert!(rendered.contains("stream failed"), "cleanup failure must be rendered: {rendered}");
}

// --- StageActorRefNotInitialized variant (Pekko parity: StageActorRefNotInitializedException) ---

#[test]
fn stage_actor_ref_not_initialized_is_constructible() {
  // Given/When: constructing StageActorRefNotInitialized
  let error = StreamError::StageActorRefNotInitialized;

  // Then: it matches the expected variant
  assert!(matches!(error, StreamError::StageActorRefNotInitialized));
}

#[test]
fn stage_actor_ref_not_initialized_display_matches_pekko_message() {
  // Given: a StageActorRefNotInitialized error
  //   Pekko reference: GraphStageLogic.StageActorRefNotInitializedException().getMessage
  //     = "You must first call getStageActor, to initialize the Actors behavior"
  let error = StreamError::StageActorRefNotInitialized;

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the message mirrors Pekko's contract exactly
  assert_eq!(rendered, "You must first call getStageActor, to initialize the Actors behavior");
}

#[test]
fn stage_actor_ref_not_initialized_is_distinct_from_actor_system_missing() {
  // Given: StageActorRefNotInitialized and ActorSystemMissing
  let not_initialized = StreamError::StageActorRefNotInitialized;
  let missing_system = StreamError::ActorSystemMissing;

  // Then: they remain distinct failure modes
  assert_ne!(not_initialized, missing_system);
}

// --- StreamLimitReached variant (Pekko parity: StreamLimitReachedException) ---

#[test]
fn stream_limit_reached_is_constructible_with_limit() {
  // Given/When: constructing StreamLimitReached with a positive limit
  let error = StreamError::StreamLimitReached { limit: 128 };

  // Then: the variant exposes the configured limit
  assert!(matches!(error, StreamError::StreamLimitReached { limit: 128 }));
}

#[test]
fn stream_limit_reached_display_matches_pekko_message() {
  // Given: a StreamLimitReached error with limit=42
  //   Pekko reference: StreamLimitReachedException(n).getMessage = "limit of $n reached"
  let error = StreamError::StreamLimitReached { limit: 42 };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the message matches the Pekko exception text exactly
  assert_eq!(rendered, "limit of 42 reached");
}

#[test]
fn stream_limit_reached_preserves_value_on_clone() {
  // Given: a StreamLimitReached error
  let original = StreamError::StreamLimitReached { limit: 7 };

  // When: cloning
  let cloned = original.clone();

  // Then: equality is preserved
  assert_eq!(original, cloned);
}

#[test]
fn stream_limit_reached_with_distinct_limits_are_unequal() {
  // Given: two StreamLimitReached errors with different limits
  let first = StreamError::StreamLimitReached { limit: 10 };
  let second = StreamError::StreamLimitReached { limit: 11 };

  // Then: they are not equal
  assert_ne!(first, second);
}

// --- WatchedActorTerminated variant (Pekko parity: WatchedActorTerminatedException) ---

#[test]
fn watched_actor_terminated_is_constructible() {
  // Given/When: constructing WatchedActorTerminated with stage name and actor path
  let error = StreamError::WatchedActorTerminated {
    watching_stage_name: Cow::Borrowed("ask"),
    actor_path:          Cow::Borrowed("pekko://system/user/target"),
  };

  // Then: it matches the expected variant
  assert!(matches!(error, StreamError::WatchedActorTerminated { .. }));
}

#[test]
fn watched_actor_terminated_display_matches_pekko_message() {
  // Given: a WatchedActorTerminated error
  //   Pekko reference: WatchedActorTerminatedException(watchingStageName, ref).getMessage
  //     = s"Actor watched by [$watchingStageName] has terminated! Was: $ref"
  let error = StreamError::WatchedActorTerminated {
    watching_stage_name: Cow::Borrowed("ask-op"),
    actor_path:          Cow::Borrowed("pekko://sys/user/target"),
  };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the formatted message mirrors Pekko's contract exactly
  assert_eq!(rendered, "Actor watched by [ask-op] has terminated! Was: pekko://sys/user/target");
}

#[test]
fn watched_actor_terminated_preserves_value_on_clone() {
  // Given: a WatchedActorTerminated error
  let original = StreamError::WatchedActorTerminated {
    watching_stage_name: Cow::Borrowed("watch"),
    actor_path:          Cow::Borrowed("pekko://sys/user/t"),
  };

  // When: cloning
  let cloned = original.clone();

  // Then: equality is preserved
  assert_eq!(original, cloned);
}

#[test]
fn watched_actor_terminated_is_distinct_from_failed() {
  // Given: both error variants
  let watched = StreamError::WatchedActorTerminated {
    watching_stage_name: Cow::Borrowed("watch"),
    actor_path:          Cow::Borrowed("pekko://sys/user/t"),
  };
  let failed = StreamError::Failed;

  // Then: they are not equal
  assert_ne!(watched, failed);
}

// --- AbruptStreamTermination variant (Pekko parity: AbruptStreamTerminationException) ---

#[test]
fn abrupt_stream_termination_is_constructible() {
  // Given/When: constructing AbruptStreamTermination with a message
  let error = StreamError::AbruptStreamTermination { message: Cow::Borrowed("materializer terminated") };

  // Then: it matches the expected variant
  assert!(matches!(error, StreamError::AbruptStreamTermination { .. }));
}

#[test]
fn abrupt_stream_termination_display_includes_message() {
  // Given: an AbruptStreamTermination error
  let error =
    StreamError::AbruptStreamTermination { message: Cow::Borrowed("processor actor [a] terminated abruptly") };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the formatted message contains the supplied detail
  assert!(rendered.contains("processor actor [a] terminated abruptly"), "unexpected rendering: {rendered}");
}

#[test]
fn abrupt_stream_termination_preserves_value_on_clone() {
  // Given: an AbruptStreamTermination error
  let original = StreamError::AbruptStreamTermination { message: Cow::Borrowed("abrupt") };

  // When: cloning
  let cloned = original.clone();

  // Then: equality is preserved
  assert_eq!(original, cloned);
}

// --- AbruptStageTermination variant (Pekko parity: AbruptStageTerminationException) ---

#[test]
fn abrupt_stage_termination_is_constructible() {
  // Given/When: constructing AbruptStageTermination with a stage name
  let error = StreamError::AbruptStageTermination { stage_name: Cow::Borrowed("CountSink") };

  // Then: it matches the expected variant
  assert!(matches!(error, StreamError::AbruptStageTermination { .. }));
}

#[test]
fn abrupt_stage_termination_display_mirrors_pekko_text() {
  // Given: an AbruptStageTermination error
  //   Pekko reference: AbruptStageTerminationException(logic).getMessage
  //     = s"GraphStage [$logic] terminated abruptly, caused by for example
  //        materializer or actor system termination."
  let error = StreamError::AbruptStageTermination { stage_name: Cow::Borrowed("CountSink") };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the message follows the Pekko phrasing verbatim
  assert_eq!(
    rendered,
    "GraphStage [CountSink] terminated abruptly, caused by for example materializer or actor system termination."
  );
}

#[test]
fn abrupt_stage_termination_is_distinct_from_stream_variant() {
  // Given: the two related but distinct variants
  let stage = StreamError::AbruptStageTermination { stage_name: Cow::Borrowed("Foo") };
  let stream = StreamError::AbruptStreamTermination { message: Cow::Borrowed("Foo") };

  // Then: they are not equal (they convey different failure levels)
  assert_ne!(stage, stream);
}

// --- CancellationKind enum (Pekko parity: SubscriptionWithCancelException.NonFailureCancellation)
// ---

#[test]
fn cancellation_kind_no_more_elements_needed_is_constructible() {
  // Given/When: constructing the NoMoreElementsNeeded variant
  let kind = CancellationKind::NoMoreElementsNeeded;

  // Then: it matches the expected variant
  assert!(matches!(kind, CancellationKind::NoMoreElementsNeeded));
}

#[test]
fn cancellation_kind_stage_was_completed_is_constructible() {
  // Given/When: constructing the StageWasCompleted variant
  let kind = CancellationKind::StageWasCompleted;

  // Then: it matches the expected variant
  assert!(matches!(kind, CancellationKind::StageWasCompleted));
}

#[test]
fn cancellation_kind_variants_are_distinct() {
  // Given: both variants
  let left = CancellationKind::NoMoreElementsNeeded;
  let right = CancellationKind::StageWasCompleted;

  // Then: they are not equal
  assert_ne!(left, right);
}

#[test]
fn cancellation_kind_is_copy() {
  // Given: a NoMoreElementsNeeded kind
  let kind = CancellationKind::NoMoreElementsNeeded;

  // When: copying
  let cloned = kind;

  // Then: equality is preserved
  assert_eq!(kind, cloned);
}

// --- CancellationCause (wraps CancellationKind — Pekko's NonFailureCancellation) ---

#[test]
fn cancellation_cause_exposes_no_more_elements_needed_kind() {
  // Given/When: constructing a CancellationCause with NoMoreElementsNeeded
  let cause = CancellationCause::no_more_elements_needed();

  // Then: the kind is retrievable
  assert_eq!(cause.kind(), CancellationKind::NoMoreElementsNeeded);
}

#[test]
fn cancellation_cause_exposes_stage_was_completed_kind() {
  // Given/When: constructing a CancellationCause with StageWasCompleted
  let cause = CancellationCause::stage_was_completed();

  // Then: the kind is retrievable
  assert_eq!(cause.kind(), CancellationKind::StageWasCompleted);
}

#[test]
fn cancellation_cause_variants_are_distinct() {
  // Given: the two canonical cancellation causes
  let no_more = CancellationCause::no_more_elements_needed();
  let completed = CancellationCause::stage_was_completed();

  // Then: they are not equal
  assert_ne!(no_more, completed);
}

#[test]
fn cancellation_cause_is_cloneable() {
  // Given: a no_more_elements_needed cause
  let original = CancellationCause::no_more_elements_needed();

  // When: cloning
  let cloned = original.clone();

  // Then: equality is preserved
  assert_eq!(original, cloned);
}

// --- StreamError::CancellationCause variant (holds a CancellationCause) ---

#[test]
fn stream_error_cancellation_cause_is_constructible_with_no_more_elements() {
  // Given/When: wrapping a NoMoreElementsNeeded cause in StreamError
  let error = StreamError::CancellationCause { cause: CancellationCause::no_more_elements_needed() };

  // Then: the error matches the expected variant
  assert!(matches!(error, StreamError::CancellationCause { .. }));
}

#[test]
fn stream_error_cancellation_cause_is_constructible_with_stage_completed() {
  // Given/When: wrapping a StageWasCompleted cause in StreamError
  let error = StreamError::CancellationCause { cause: CancellationCause::stage_was_completed() };

  // Then: the error matches the expected variant
  assert!(matches!(error, StreamError::CancellationCause { .. }));
}

#[test]
fn stream_error_cancellation_cause_display_contains_kind_marker() {
  // Given: a CancellationCause error wrapping NoMoreElementsNeeded
  //   Pekko reference: SubscriptionWithCancelException.NoMoreElementsNeeded case object
  let error = StreamError::CancellationCause { cause: CancellationCause::no_more_elements_needed() };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the rendering includes a recognizable marker for the specific kind
  assert!(
    rendered.contains("NoMoreElementsNeeded") || rendered.contains("no more elements"),
    "unexpected rendering: {rendered}"
  );
}

#[test]
fn stream_error_cancellation_cause_kinds_are_distinguishable() {
  // Given: two CancellationCause errors with different kinds
  let no_more = StreamError::CancellationCause { cause: CancellationCause::no_more_elements_needed() };
  let completed = StreamError::CancellationCause { cause: CancellationCause::stage_was_completed() };

  // Then: they are not equal at the StreamError level
  assert_ne!(no_more, completed);
}

#[test]
fn stream_error_cancellation_cause_preserves_value_on_clone() {
  // Given: a CancellationCause error
  let original = StreamError::CancellationCause { cause: CancellationCause::no_more_elements_needed() };

  // When: cloning
  let cloned = original.clone();

  // Then: equality is preserved
  assert_eq!(original, cloned);
}

// --- Display rendering for coexisting variants stays stable ---

#[test]
fn existing_variants_keep_display_contract_after_additions() {
  // Given: pre-existing variants that older tests already cover
  let stopped = StreamError::MaterializerStopped;

  // When: formatting with Display
  let rendered = alloc::format!("{stopped}");

  // Then: the previous wording is preserved (regression guard for the new variants)
  assert_eq!(rendered, "materializer stopped");
}

// --- StreamRef error variants (Pekko parity: StreamRefs.scala exceptions) ---

#[test]
fn stream_ref_target_not_initialized_is_constructible() {
  // Given/When: constructing StreamRefTargetNotInitialized
  let error = StreamError::StreamRefTargetNotInitialized;

  // Then: the variant is available for StreamRef protocol failures
  assert!(matches!(error, StreamError::StreamRefTargetNotInitialized));
}

#[test]
fn stream_ref_target_not_initialized_display_contains_reference_message() {
  // Given: target ref not initialized error
  let error = StreamError::StreamRefTargetNotInitialized;

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the message mirrors Pekko's diagnostic intent
  assert!(rendered.contains("target actor ref not yet resolved"), "unexpected rendering: {rendered}");
}

#[test]
fn stream_ref_subscription_timeout_preserves_message() {
  // Given: StreamRef subscription timeout with a diagnostic message
  let error = StreamError::StreamRefSubscriptionTimeout { message: Cow::Borrowed("remote side did not subscribe") };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the supplied message is used as the error text
  assert_eq!(rendered, "remote side did not subscribe");
}

#[test]
fn remote_stream_ref_actor_terminated_preserves_message() {
  // Given: remote StreamRef actor termination with a diagnostic message
  let error = StreamError::RemoteStreamRefActorTerminated { message: Cow::Borrowed("remote actor stopped") };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the supplied message is used as the error text
  assert_eq!(rendered, "remote actor stopped");
}

#[test]
fn invalid_sequence_number_display_includes_sequence_context() {
  // Given: invalid StreamRef sequence number
  let error = StreamError::InvalidSequenceNumber {
    expected_seq_nr: 10,
    got_seq_nr:      9,
    message:         Cow::Borrowed("invalid seq"),
  };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: expected/got values and Pekko message-loss hint are visible
  assert!(rendered.contains("invalid seq"), "message must be preserved: {rendered}");
  assert!(rendered.contains("expected: 10"), "expected sequence must be rendered: {rendered}");
  assert!(rendered.contains("got: 9"), "received sequence must be rendered: {rendered}");
  assert!(rendered.contains("message loss"), "Pekko diagnostic hint must be rendered: {rendered}");
}

#[test]
fn invalid_partner_actor_display_includes_actor_context() {
  // Given: message from a non-partner actor
  let error = StreamError::InvalidPartnerActor {
    expected_ref: Cow::Borrowed("pekko://sys/user/expected"),
    got_ref:      Cow::Borrowed("pekko://sys/user/got"),
    message:      Cow::Borrowed("invalid partner"),
  };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: expected/got actor refs and one-shot StreamRef guidance are visible
  assert!(rendered.contains("invalid partner"), "message must be preserved: {rendered}");
  assert!(rendered.contains("expected: pekko://sys/user/expected"), "expected ref must be rendered: {rendered}");
  assert!(rendered.contains("got: pekko://sys/user/got"), "got ref must be rendered: {rendered}");
  assert!(rendered.contains("one-shot references"), "Pekko one-shot guidance must be rendered: {rendered}");
}

#[test]
fn stream_ref_error_variants_are_distinct() {
  // Given: StreamRef-specific error variants
  let timeout = StreamError::StreamRefSubscriptionTimeout { message: Cow::Borrowed("x") };
  let terminated = StreamError::RemoteStreamRefActorTerminated { message: Cow::Borrowed("x") };
  let target = StreamError::StreamRefTargetNotInitialized;

  // Then: superficially similar diagnostics remain distinct enum variants
  assert_ne!(timeout, terminated);
  assert_ne!(timeout, target);
  assert_ne!(terminated, target);
}

#[test]
fn invalid_sequence_number_clone_preserves_fields() {
  // Given: invalid sequence number error
  let original = StreamError::InvalidSequenceNumber {
    expected_seq_nr: 10,
    got_seq_nr:      9,
    message:         Cow::Borrowed("invalid seq"),
  };

  // When: cloning
  let cloned = original.clone();

  // Then: all fields round-trip through Clone/Eq
  assert_eq!(original, cloned);
}

// --- TooManySubstreamsOpen variant (Pekko parity: TooManySubstreamsOpenException) ---

#[test]
fn too_many_substreams_open_is_constructible_with_max() {
  // Given/When: constructing TooManySubstreamsOpen with a max_substreams value
  //   Pekko reference: TooManySubstreamsOpenException — argumentless on the JVM side,
  //   but fraktor-rs preserves max_substreams for Debug/diagnostic purposes.
  let error = StreamError::TooManySubstreamsOpen { max_substreams: 100 };

  // Then: the variant exposes the configured max
  assert!(matches!(error, StreamError::TooManySubstreamsOpen { max_substreams: 100 }));
}

#[test]
fn too_many_substreams_open_display_matches_pekko_message_verbatim() {
  // Given: a TooManySubstreamsOpen error with an arbitrary max_substreams value
  //   Pekko reference:
  //     class TooManySubstreamsOpenException
  //         extends IllegalStateException("Cannot open a new substream as there are too many
  // substreams open")
  let error = StreamError::TooManySubstreamsOpen { max_substreams: 100 };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the message matches Pekko verbatim (max_substreams is intentionally NOT in Display)
  assert_eq!(rendered, "Cannot open a new substream as there are too many substreams open");
}

#[test]
fn too_many_substreams_open_display_omits_max_substreams_value() {
  // Given: a TooManySubstreamsOpen error with a distinctive max_substreams value
  let error = StreamError::TooManySubstreamsOpen { max_substreams: 9999 };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the max_substreams numeric value does NOT leak into the Display output
  //   (Pekko's exception message is fixed; the value lives only in Debug.)
  assert!(!rendered.contains("9999"), "Display must not include max_substreams value: {rendered}");
  assert!(!rendered.contains("max_substreams"), "Display must not include max_substreams field name: {rendered}");
}

#[test]
fn too_many_substreams_open_display_independent_of_max_value() {
  // Given: two TooManySubstreamsOpen errors with very different max_substreams values
  let small = StreamError::TooManySubstreamsOpen { max_substreams: 1 };
  let large = StreamError::TooManySubstreamsOpen { max_substreams: 1_000_000 };

  // When: formatting both with Display
  let small_rendered = alloc::format!("{small}");
  let large_rendered = alloc::format!("{large}");

  // Then: both render to the identical Pekko-fixed message
  assert_eq!(small_rendered, large_rendered);
  assert_eq!(small_rendered, "Cannot open a new substream as there are too many substreams open");
}

#[test]
fn too_many_substreams_open_preserves_value_on_clone() {
  // Given: a TooManySubstreamsOpen error
  let original = StreamError::TooManySubstreamsOpen { max_substreams: 7 };

  // When: cloning
  let cloned = original.clone();

  // Then: equality is preserved (max_substreams round-trips for diagnostic use)
  assert_eq!(original, cloned);
}

#[test]
fn too_many_substreams_open_with_distinct_max_values_are_unequal() {
  // Given: two TooManySubstreamsOpen errors with different max_substreams
  //   Display is identical, but PartialEq still discriminates by field value.
  let first = StreamError::TooManySubstreamsOpen { max_substreams: 10 };
  let second = StreamError::TooManySubstreamsOpen { max_substreams: 11 };

  // Then: they are not equal at the StreamError level
  assert_ne!(first, second);
}

#[test]
fn too_many_substreams_open_debug_includes_max_substreams_for_diagnostics() {
  // Given: a TooManySubstreamsOpen error with a distinctive max_substreams value
  //   The numeric value is dropped from Display (Pekko parity) but must still be
  //   recoverable from Debug to keep the field useful for diagnostics.
  let error = StreamError::TooManySubstreamsOpen { max_substreams: 4242 };

  // When: formatting with Debug
  let debug = alloc::format!("{error:?}");

  // Then: the Debug output retains the value
  assert!(debug.contains("4242"), "Debug should expose max_substreams value: {debug}");
}

// ---------------------------------------------------------------------------
// Framing variant
//   Pekko parity: `pekko.stream.scaladsl.Framing.FramingException(msg)` (Framing.scala:159).
//   fraktor-rs lifts the free-form message into a typed sub-enum
//   (`FramingErrorKind`) so that `FrameTooLarge` and `Malformed` can be
//   discriminated without string parsing. StreamError::Framing delegates
//   Display to the inner kind, preserving Pekko-shaped diagnostics verbatim.
// ---------------------------------------------------------------------------

#[test]
fn framing_is_constructible_with_frame_too_large() {
  // Given/When: constructing Framing with a FrameTooLarge kind
  let error = StreamError::Framing { kind: FramingErrorKind::FrameTooLarge { actual: 2048, max: 1024 } };

  // Then: the variant matches at both the outer and the inner level
  assert!(matches!(error, StreamError::Framing { kind: FramingErrorKind::FrameTooLarge { actual: 2048, max: 1024 } }));
}

#[test]
fn framing_is_constructible_with_malformed() {
  // Given/When: constructing Framing with a Malformed kind wrapping a diagnostic string
  let error = StreamError::Framing { kind: FramingErrorKind::Malformed(String::from("truncated")) };

  // Then: the variant matches
  assert!(matches!(error, StreamError::Framing { kind: FramingErrorKind::Malformed(_) }));
}

#[test]
fn framing_display_delegates_to_kind_for_frame_too_large() {
  // Given: a Framing wrapping a FrameTooLarge with distinctive numeric fields
  //   Pekko reference message: "Maximum allowed message size is $max but tried
  //                             to send $actual bytes" (Framing.scala:196).
  let error = StreamError::Framing { kind: FramingErrorKind::FrameTooLarge { actual: 2048, max: 1024 } };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: both the actual and the max sizes are present (delegated through the inner kind)
  assert!(rendered.contains("2048"), "rendered output must contain actual size: {rendered}");
  assert!(rendered.contains("1024"), "rendered output must contain max size: {rendered}");
}

#[test]
fn framing_display_delegates_to_kind_for_malformed() {
  // Given: a Framing wrapping a Malformed with a Pekko-style diagnostic message
  //   Pekko reference: "Stream finished but there was a truncated final frame
  //                     in the buffer" (Framing.scala:256).
  let msg = "Stream finished but there was a truncated final frame in the buffer";
  let error = StreamError::Framing { kind: FramingErrorKind::Malformed(String::from(msg)) };

  // When: formatting with Display
  let rendered = alloc::format!("{error}");

  // Then: the full diagnostic message appears verbatim (delegated through the inner kind)
  assert!(rendered.contains(msg), "rendered output must contain the original message verbatim: {rendered}");
}

#[test]
fn framing_variants_are_mutually_distinct() {
  // Given: Framing wrapping each of the two FramingErrorKind variants
  let too_large = StreamError::Framing { kind: FramingErrorKind::FrameTooLarge { actual: 1, max: 0 } };
  let malformed = StreamError::Framing { kind: FramingErrorKind::Malformed(String::from("x")) };

  // Then: they are not equal at the StreamError level
  //   (discrimination must be preserved through the wrapping).
  assert_ne!(too_large, malformed);
}

#[test]
fn framing_clone_preserves_variant() {
  // Given: a Framing error
  let original = StreamError::Framing { kind: FramingErrorKind::FrameTooLarge { actual: 10, max: 5 } };

  // When: cloning
  let cloned = original.clone();

  // Then: equality is preserved through Clone
  assert_eq!(original, cloned);
}

#[test]
fn framing_is_distinct_from_io_error() {
  // Given: a Framing error AND an IoError with superficially similar text
  //   Both variants can carry a descriptive string, but the enum must still
  //   discriminate them so callers can pattern-match the root cause.
  let framing = StreamError::Framing { kind: FramingErrorKind::Malformed(String::from("truncated")) };
  let io = StreamError::IoError { kind: String::from("UnexpectedEof"), message: String::from("truncated") };

  // Then: they are not equal at the enum level
  assert_ne!(framing, io);
}
