//! Codec for [`RemoteDeploymentPdu`].

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::wire::{
  Codec, RemoteDeploymentCreateFailure, RemoteDeploymentCreateRequest, RemoteDeploymentCreateSuccess,
  RemoteDeploymentFailureCode, RemoteDeploymentPdu, WireError,
  frame_header::KIND_DEPLOYMENT,
  primitives::{
    begin_frame, decode_bytes, decode_option_string, decode_string, encode_bytes, encode_option_string, encode_string,
    patch_frame_length, read_frame_header,
  },
};

const TAG_CREATE_REQUEST: u8 = 0x01;
const TAG_CREATE_SUCCESS: u8 = 0x02;
const TAG_CREATE_FAILURE: u8 = 0x03;

/// Zero-sized codec for [`RemoteDeploymentPdu`].
#[derive(Clone, Copy, Debug, Default)]
pub struct RemoteDeploymentCodec;

impl RemoteDeploymentCodec {
  /// Creates a new [`RemoteDeploymentCodec`].
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Codec<RemoteDeploymentPdu> for RemoteDeploymentCodec {
  fn encode(&self, value: &RemoteDeploymentPdu, buf: &mut BytesMut) -> Result<(), WireError> {
    let len_pos = begin_frame(buf, KIND_DEPLOYMENT);
    match value {
      | RemoteDeploymentPdu::CreateRequest(request) => encode_create_request(request, buf)?,
      | RemoteDeploymentPdu::CreateSuccess(success) => encode_create_success(success, buf)?,
      | RemoteDeploymentPdu::CreateFailure(failure) => encode_create_failure(failure, buf)?,
    }
    patch_frame_length(buf, len_pos)
  }

  fn decode(&self, buf: &mut Bytes) -> Result<RemoteDeploymentPdu, WireError> {
    read_frame_header(buf, KIND_DEPLOYMENT)?;
    if buf.remaining() < 1 {
      return Err(WireError::Truncated);
    }
    match buf.get_u8() {
      | TAG_CREATE_REQUEST => decode_create_request(buf).map(RemoteDeploymentPdu::CreateRequest),
      | TAG_CREATE_SUCCESS => decode_create_success(buf).map(RemoteDeploymentPdu::CreateSuccess),
      | TAG_CREATE_FAILURE => decode_create_failure(buf).map(RemoteDeploymentPdu::CreateFailure),
      | _ => Err(WireError::InvalidFormat),
    }
  }
}

fn encode_create_request(request: &RemoteDeploymentCreateRequest, buf: &mut BytesMut) -> Result<(), WireError> {
  buf.put_u8(TAG_CREATE_REQUEST);
  encode_correlation(request.correlation_hi(), request.correlation_lo(), buf);
  encode_string(request.target_parent_path(), buf)?;
  encode_string(request.child_name(), buf)?;
  encode_string(request.factory_id(), buf)?;
  encode_string(request.origin_node(), buf)?;
  buf.put_u32(request.serializer_id());
  encode_option_string(request.manifest(), buf)?;
  encode_bytes(request.payload(), buf)
}

fn encode_create_success(success: &RemoteDeploymentCreateSuccess, buf: &mut BytesMut) -> Result<(), WireError> {
  buf.put_u8(TAG_CREATE_SUCCESS);
  encode_correlation(success.correlation_hi(), success.correlation_lo(), buf);
  encode_string(success.actor_path(), buf)
}

fn encode_create_failure(failure: &RemoteDeploymentCreateFailure, buf: &mut BytesMut) -> Result<(), WireError> {
  buf.put_u8(TAG_CREATE_FAILURE);
  encode_correlation(failure.correlation_hi(), failure.correlation_lo(), buf);
  buf.put_u8(failure.code().to_wire());
  encode_string(failure.reason(), buf)
}

fn decode_create_request(buf: &mut Bytes) -> Result<RemoteDeploymentCreateRequest, WireError> {
  let (correlation_hi, correlation_lo) = decode_correlation(buf)?;
  let target_parent_path = decode_string(buf)?;
  let child_name = decode_string(buf)?;
  let factory_id = decode_string(buf)?;
  let origin_node = decode_string(buf)?;
  if buf.remaining() < 4 {
    return Err(WireError::Truncated);
  }
  let serializer_id = buf.get_u32();
  let manifest = decode_option_string(buf)?;
  let payload = decode_bytes(buf)?;
  Ok(RemoteDeploymentCreateRequest::new(
    correlation_hi,
    correlation_lo,
    target_parent_path,
    child_name,
    factory_id,
    origin_node,
    serializer_id,
    manifest,
    payload,
  ))
}

fn decode_create_success(buf: &mut Bytes) -> Result<RemoteDeploymentCreateSuccess, WireError> {
  let (correlation_hi, correlation_lo) = decode_correlation(buf)?;
  let actor_path = decode_string(buf)?;
  Ok(RemoteDeploymentCreateSuccess::new(correlation_hi, correlation_lo, actor_path))
}

fn decode_create_failure(buf: &mut Bytes) -> Result<RemoteDeploymentCreateFailure, WireError> {
  let (correlation_hi, correlation_lo) = decode_correlation(buf)?;
  if buf.remaining() < 1 {
    return Err(WireError::Truncated);
  }
  let Some(code) = RemoteDeploymentFailureCode::from_wire(buf.get_u8()) else {
    return Err(WireError::InvalidFormat);
  };
  let reason = decode_string(buf)?;
  Ok(RemoteDeploymentCreateFailure::new(correlation_hi, correlation_lo, code, reason))
}

fn encode_correlation(correlation_hi: u64, correlation_lo: u32, buf: &mut BytesMut) {
  buf.put_u64(correlation_hi);
  buf.put_u32(correlation_lo);
}

fn decode_correlation(buf: &mut Bytes) -> Result<(u64, u32), WireError> {
  if buf.remaining() < 8 + 4 {
    return Err(WireError::Truncated);
  }
  Ok((buf.get_u64(), buf.get_u32()))
}
