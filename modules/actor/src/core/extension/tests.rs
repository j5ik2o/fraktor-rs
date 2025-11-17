use core::any::TypeId;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use super::{Extension, ExtensionId};
use crate::core::system::ActorSystemGeneric;

struct DummyExt;
impl<TB: RuntimeToolbox> Extension<TB> for DummyExt {}

struct DummyId;

impl<TB: RuntimeToolbox> ExtensionId<TB> for DummyId {
  type Ext = DummyExt;

  fn create_extension(&self, _system: &ActorSystemGeneric<TB>) -> Self::Ext {
    DummyExt
  }
}

#[test]
fn type_id_is_stable() {
  let id = DummyId;
  assert_eq!(<DummyId as ExtensionId<NoStdToolbox>>::id(&id), TypeId::of::<DummyId>());
}

#[test]
fn factory_creates_extension() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let id = DummyId;
  let ext = id.create_extension(&system);
  let _shared: ArcShared<DummyExt> = ArcShared::new(ext);
}
