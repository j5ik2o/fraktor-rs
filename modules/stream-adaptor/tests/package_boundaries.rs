use core::any::TypeId;

use fraktor_stream_adaptor_rs::std::{
  io::{FileIO, SourceFactory, StreamConverters},
  materializer::{SystemMaterializer, SystemMaterializerId},
};

#[test]
fn std_packages_export_io_and_materializer_adapters() {
  assert_eq!(TypeId::of::<FileIO>(), TypeId::of::<FileIO>());
  assert_eq!(TypeId::of::<SourceFactory>(), TypeId::of::<SourceFactory>());
  assert_eq!(TypeId::of::<StreamConverters>(), TypeId::of::<StreamConverters>());
  assert_eq!(TypeId::of::<SystemMaterializer>(), TypeId::of::<SystemMaterializer>());
  assert_eq!(TypeId::of::<SystemMaterializerId>(), TypeId::of::<SystemMaterializerId>());
}
