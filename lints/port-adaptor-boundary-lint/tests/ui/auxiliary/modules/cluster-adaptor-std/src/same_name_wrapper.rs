use fraktor_cluster_core_kernel_rs::grain::GrainRef as CoreGrainRef;
use fraktor_cluster_core_kernel_rs::extension::ClusterApi as CoreClusterApi;

pub struct GrainRef {
  inner: CoreGrainRef,
}

pub struct ClusterApi {
  inner: CoreClusterApi,
}

pub struct FullyQualifiedGrainRef {
  inner: fraktor_cluster_core_kernel_rs::grain::FullyQualifiedGrainRef,
}
