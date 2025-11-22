use crate::core::rpc_error::RpcError;

#[test]
fn schema_mismatch_carries_versions() {
  let err = RpcError::SchemaMismatch { negotiated: Some(2), message_version: 1 };
  assert_eq!(err, RpcError::SchemaMismatch { negotiated: Some(2), message_version: 1 });
}
