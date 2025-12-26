use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::core::ActorMaterializerGeneric;

/// Actor materializer specialised for `StdToolbox`.
pub type ActorMaterializer = ActorMaterializerGeneric<StdToolbox>;
