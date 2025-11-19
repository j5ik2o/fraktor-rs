//! Serialization setup utilities for remoting examples and testing.

use alloc::string::String;

use fraktor_actor_rs::core::serialization::{
  SerializationCallScope, SerializationSetup, SerializationSetupBuilder, SerializerId, StringSerializer,
};
use fraktor_utils_rs::core::sync::ArcShared;

/// Creates a default serialization setup for loopback examples using String serializer only.
///
/// This setup is suitable for simple loopback examples and testing, but should not be used
/// in production environments. For production use, create a custom serialization setup
/// with appropriate serializers for your message types.
///
/// # Panics
///
/// Panics if the serializer registration or binding fails, which should not happen under
/// normal conditions.
#[must_use]
pub fn default_loopback_setup() -> SerializationSetup {
  let serializer_id = SerializerId::try_from(81).expect("serializer id");
  let serializer: ArcShared<dyn fraktor_actor_rs::core::serialization::Serializer> =
    ArcShared::new(StringSerializer::new(serializer_id));
  SerializationSetupBuilder::new()
    .register_serializer("string", serializer_id, serializer)
    .expect("register serializer")
    .bind::<String>("string")
    .expect("bind string")
    .bind_remote_manifest::<String>("remote.String")
    .expect("manifest")
    .set_fallback("string")
    .expect("fallback")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("build setup")
}
