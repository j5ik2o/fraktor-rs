use core::any::TypeId;

use fraktor_stream_adaptor_std_rs::{
  io::{FileIO, SourceFactory, StreamConverters},
  materializer::{SystemMaterializer, SystemMaterializerId},
};

#[test]
fn std_packages_export_io_and_materializer_adapters() {
  let _ = TypeId::of::<FileIO>();
  let _ = TypeId::of::<SourceFactory>();
  let _ = TypeId::of::<StreamConverters>();
  let _ = TypeId::of::<SystemMaterializer>();
  let _ = TypeId::of::<SystemMaterializerId>();
}
