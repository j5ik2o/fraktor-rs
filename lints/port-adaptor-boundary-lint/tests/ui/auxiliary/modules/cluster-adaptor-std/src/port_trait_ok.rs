use fraktor_cluster_core_kernel_rs::extension::ClusterIdentityResolver;

pub struct PubSubDeliveryActor {
  identity_resolver: Box<dyn ClusterIdentityResolver>,
}
