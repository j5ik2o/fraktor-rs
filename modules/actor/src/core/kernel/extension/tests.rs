use core::any::TypeId;

use fraktor_utils_rs::core::sync::ArcShared;

use super::{Extension, ExtensionId};
use crate::core::kernel::system::ActorSystem;

struct DummyExt;
impl Extension for DummyExt {}

struct DummyId;

impl ExtensionId for DummyId {
  type Ext = DummyExt;

  fn create_extension(&self, _system: &ActorSystem) -> Self::Ext {
    DummyExt
  }
}

#[test]
fn type_id_is_stable() {
  let id = DummyId;
  assert_eq!(<DummyId as ExtensionId>::id(&id), TypeId::of::<DummyId>());
}

#[test]
fn factory_creates_extension() {
  let system = ActorSystem::new_empty();
  let id = DummyId;
  let ext = id.create_extension(&system);
  let _shared: ArcShared<DummyExt> = ArcShared::new(ext);
}
