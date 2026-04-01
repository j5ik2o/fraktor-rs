use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  kernel::{
    actor::{
      extension::{Extension, ExtensionId},
      scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
    },
    system::ActorSystem,
  },
  typed::{TypedActorSystem, TypedProps, dsl::Behaviors},
};

struct TestExtension {
  value: u32,
}

impl Extension for TestExtension {}

struct TestExtensionId {
  initial_value: u32,
}

impl ExtensionId for TestExtensionId {
  type Ext = TestExtension;

  fn create_extension(&self, _system: &ActorSystem) -> Self::Ext {
    TestExtension { value: self.initial_value }
  }
}

fn new_test_system() -> TypedActorSystem<u32> {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system")
}

// --- T5: Extension facade tests ---

#[test]
fn register_extension_returns_created_instance() {
  // Given: a typed actor system and an extension id
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 42 };

  // When: register_extension is called
  let ext = system.register_extension(&ext_id);

  // Then: the created extension is returned with the initial value
  assert_eq!(ext.value, 42);

  system.terminate().expect("terminate");
}

#[test]
fn has_extension_returns_false_before_registration() {
  // Given: a typed actor system with no extensions registered
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 0 };

  // When/Then: has_extension returns false
  assert!(!system.has_extension(&ext_id));

  system.terminate().expect("terminate");
}

#[test]
fn has_extension_returns_true_after_registration() {
  // Given: a typed actor system with an extension registered
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 0 };
  system.register_extension(&ext_id);

  // When/Then: has_extension returns true
  assert!(system.has_extension(&ext_id));

  system.terminate().expect("terminate");
}

#[test]
fn extension_returns_none_before_registration() {
  // Given: a typed actor system with no extensions registered
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 0 };

  // When/Then: extension returns None
  let result: Option<ArcShared<TestExtension>> = system.extension(&ext_id);
  assert!(result.is_none());

  system.terminate().expect("terminate");
}

#[test]
fn extension_returns_registered_instance() {
  // Given: a typed actor system with an extension registered
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 99 };
  system.register_extension(&ext_id);

  // When: extension is called
  let result: Option<ArcShared<TestExtension>> = system.extension(&ext_id);

  // Then: the registered instance is returned
  let ext = result.expect("extension should be present");
  assert_eq!(ext.value, 99);

  system.terminate().expect("terminate");
}

#[test]
fn register_extension_is_idempotent() {
  // Given: a typed actor system with an extension already registered
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 10 };
  let first = system.register_extension(&ext_id);

  // When: register_extension is called again with the same id
  let second = system.register_extension(&ext_id);

  // Then: the same instance is returned (putIfAbsent semantics)
  assert_eq!(first.value, second.value);

  system.terminate().expect("terminate");
}
