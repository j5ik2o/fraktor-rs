//! ActorSystem integration and cluster entrypoints for std runtimes.

#[cfg(feature = "aws-ecs")]
mod aws_ecs_cluster_extension_installer_ext;

#[cfg(feature = "aws-ecs")]
pub use aws_ecs_cluster_extension_installer_ext::AwsEcsClusterExtensionInstallerExt;
