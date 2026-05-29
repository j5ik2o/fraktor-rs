use fraktor_cluster_core_kernel_rs::grain::GrainRef as CoreGrainRef;

pub struct GrainRef {
  inner: CoreGrainRef,
}
