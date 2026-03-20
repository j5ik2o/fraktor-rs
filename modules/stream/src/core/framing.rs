use alloc::{boxed::Box, vec::Vec};
use core::any::TypeId;

use super::{
  DynValue, FlowDefinition, FlowLogic, MatCombine, StageDefinition, StreamError, StreamNotUsed, SupervisionStrategy,
  downcast_value,
  graph::StreamGraph,
  shape::{Inlet, Outlet},
  stage::{BidiFlow, StageKind, flow::Flow},
};

#[cfg(test)]
mod tests;

const SIMPLE_FRAMING_LENGTH_FIELD_SIZE: usize = 4;

/// Byte stream framing utilities.
///
/// Provides factory methods that produce flows splitting byte streams
/// into frames based on delimiters or length fields.
pub struct Framing;

impl Framing {
  /// Creates a flow that splits byte streams on a delimiter.
  ///
  /// Each input `Vec<u8>` chunk is accumulated into an internal buffer.
  /// Complete frames (terminated by `delimiter`) are emitted downstream.
  /// Frames exceeding `max_frame_length` cause a [`StreamError`].
  /// When `allow_truncation` is true, remaining bytes without a trailing
  /// delimiter are emitted on source completion.
  #[must_use]
  pub fn delimiter(
    delimiter: Vec<u8>,
    max_frame_length: usize,
    allow_truncation: bool,
  ) -> Flow<Vec<u8>, Vec<u8>, StreamNotUsed> {
    let definition = delimiter_framing_definition(delimiter, max_frame_length, allow_truncation);
    let mut graph = StreamGraph::new();
    graph.push_stage(StageDefinition::Flow(definition));
    Flow::from_graph(graph, StreamNotUsed::new())
  }

  /// Creates a flow that splits byte streams based on a length field.
  ///
  /// Each input `Vec<u8>` chunk is accumulated. The length field at
  /// `field_offset` (big-endian, `field_length` bytes) determines the
  /// total frame size. Complete frames are emitted downstream.
  #[must_use]
  pub fn length_field(field_offset: usize, field_length: usize) -> Flow<Vec<u8>, Vec<u8>, StreamNotUsed> {
    Flow::new().stateful_map_concat(move || {
      let mut buffer: Vec<u8> = Vec::new();
      move |chunk: Vec<u8>| {
        buffer.extend_from_slice(&chunk);
        let mut frames: Vec<Vec<u8>> = Vec::new();
        loop {
          let header_end = field_offset + field_length;
          if buffer.len() < header_end {
            break;
          }
          let payload_len = read_big_endian_uint(&buffer[field_offset..header_end]);
          let frame_len = header_end + payload_len;
          if buffer.len() < frame_len {
            break;
          }
          let frame: Vec<u8> = buffer[..frame_len].to_vec();
          buffer = buffer[frame_len..].to_vec();
          frames.push(frame);
        }
        frames
      }
    })
  }

  /// Creates a bidirectional framing protocol with a 4-byte big-endian length prefix.
  #[must_use]
  pub fn simple_framing_protocol(
    maximum_message_length: usize,
  ) -> BidiFlow<Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, StreamNotUsed> {
    BidiFlow::from_flows_mat(
      simple_framing_encoder(maximum_message_length),
      simple_framing_decoder(maximum_message_length),
      StreamNotUsed::new(),
    )
  }
}

struct DelimiterFramingLogic {
  delimiter:        Vec<u8>,
  max_frame_length: usize,
  allow_truncation: bool,
  buffer:           Vec<u8>,
  source_done:      bool,
}

impl FlowLogic for DelimiterFramingLogic {
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let chunk = downcast_value::<Vec<u8>>(input)?;
    self.buffer.extend_from_slice(&chunk);
    let mut frames: Vec<DynValue> = Vec::new();
    loop {
      let Some(pos) = find_delimiter(&self.buffer, &self.delimiter) else {
        break;
      };
      let frame: Vec<u8> = self.buffer[..pos].to_vec();
      let new_start = pos + self.delimiter.len();
      self.buffer = self.buffer[new_start..].to_vec();
      if frame.len() > self.max_frame_length {
        return Err(StreamError::BufferOverflow);
      }
      frames.push(Box::new(frame) as DynValue);
    }
    if self.buffer.len() > self.max_frame_length {
      return Err(StreamError::BufferOverflow);
    }
    Ok(frames)
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if self.allow_truncation && self.source_done && !self.buffer.is_empty() {
      let remaining = core::mem::take(&mut self.buffer);
      return Ok(alloc::vec![Box::new(remaining) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn has_pending_output(&self) -> bool {
    self.allow_truncation && self.source_done && !self.buffer.is_empty()
  }
}

struct SimpleFramingEncoderLogic {
  maximum_message_length: usize,
}

impl FlowLogic for SimpleFramingEncoderLogic {
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let payload = downcast_value::<Vec<u8>>(input)?;
    if payload.len() > self.maximum_message_length || payload.len() > u32::MAX as usize {
      return Err(StreamError::BufferOverflow);
    }

    let frame_len = checked_frame_length(SIMPLE_FRAMING_LENGTH_FIELD_SIZE, payload.len())?;
    let mut frame = Vec::with_capacity(frame_len);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(alloc::vec![Box::new(frame) as DynValue])
  }
}

struct SimpleFramingDecoderLogic {
  maximum_message_length: usize,
  buffer:                 Vec<u8>,
}

impl SimpleFramingDecoderLogic {
  fn decode_frames(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let mut frames: Vec<DynValue> = Vec::new();

    loop {
      if self.buffer.len() < SIMPLE_FRAMING_LENGTH_FIELD_SIZE {
        return Ok(frames);
      }

      let payload_len = read_big_endian_i32([self.buffer[0], self.buffer[1], self.buffer[2], self.buffer[3]]);
      if payload_len < 0 {
        return Err(StreamError::Failed);
      }
      let payload_len = payload_len as usize;
      if payload_len > self.maximum_message_length {
        return Err(StreamError::BufferOverflow);
      }

      let frame_len = checked_frame_length(SIMPLE_FRAMING_LENGTH_FIELD_SIZE, payload_len)?;
      if self.buffer.len() < frame_len {
        return Ok(frames);
      }

      let payload = self.buffer[SIMPLE_FRAMING_LENGTH_FIELD_SIZE..frame_len].to_vec();
      self.buffer = self.buffer[frame_len..].to_vec();
      frames.push(Box::new(payload) as DynValue);
    }
  }
}

impl FlowLogic for SimpleFramingDecoderLogic {
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let chunk = downcast_value::<Vec<u8>>(input)?;
    self.buffer.extend_from_slice(&chunk);
    self.decode_frames()
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    if self.buffer.is_empty() {
      return Ok(());
    }
    Err(StreamError::Failed)
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    Ok(Vec::new())
  }

  fn has_pending_output(&self) -> bool {
    false
  }
}

fn delimiter_framing_definition(delimiter: Vec<u8>, max_frame_length: usize, allow_truncation: bool) -> FlowDefinition {
  let inlet: Inlet<Vec<u8>> = Inlet::new();
  let outlet: Outlet<Vec<u8>> = Outlet::new();
  let logic =
    DelimiterFramingLogic { delimiter, max_frame_length, allow_truncation, buffer: Vec::new(), source_done: false };
  FlowDefinition {
    kind:        StageKind::FlowStatefulMapConcat,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<Vec<u8>>(),
    output_type: TypeId::of::<Vec<u8>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

fn simple_framing_encoder(maximum_message_length: usize) -> Flow<Vec<u8>, Vec<u8>, StreamNotUsed> {
  let mut graph = StreamGraph::new();
  graph.push_stage(StageDefinition::Flow(simple_framing_encoder_definition(maximum_message_length)));
  Flow::from_graph(graph, StreamNotUsed::new())
}

fn simple_framing_encoder_definition(maximum_message_length: usize) -> FlowDefinition {
  let inlet: Inlet<Vec<u8>> = Inlet::new();
  let outlet: Outlet<Vec<u8>> = Outlet::new();
  FlowDefinition {
    kind:        StageKind::FlowStatefulMapConcat,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<Vec<u8>>(),
    output_type: TypeId::of::<Vec<u8>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(SimpleFramingEncoderLogic { maximum_message_length }),
  }
}

fn simple_framing_decoder(maximum_message_length: usize) -> Flow<Vec<u8>, Vec<u8>, StreamNotUsed> {
  let mut graph = StreamGraph::new();
  graph.push_stage(StageDefinition::Flow(simple_framing_decoder_definition(maximum_message_length)));
  Flow::from_graph(graph, StreamNotUsed::new())
}

fn simple_framing_decoder_definition(maximum_message_length: usize) -> FlowDefinition {
  let inlet: Inlet<Vec<u8>> = Inlet::new();
  let outlet: Outlet<Vec<u8>> = Outlet::new();
  FlowDefinition {
    kind:        StageKind::FlowStatefulMapConcat,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<Vec<u8>>(),
    output_type: TypeId::of::<Vec<u8>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(SimpleFramingDecoderLogic { maximum_message_length, buffer: Vec::new() }),
  }
}

fn find_delimiter(haystack: &[u8], needle: &[u8]) -> Option<usize> {
  if needle.is_empty() || haystack.len() < needle.len() {
    return None;
  }
  haystack.windows(needle.len()).position(|window| window == needle)
}

fn checked_frame_length(header_len: usize, payload_len: usize) -> Result<usize, StreamError> {
  header_len.checked_add(payload_len).ok_or(StreamError::BufferOverflow)
}

fn read_big_endian_uint(bytes: &[u8]) -> usize {
  let mut value: usize = 0;
  for &byte in bytes {
    value = (value << 8) | usize::from(byte);
  }
  value
}

const fn read_big_endian_i32(bytes: [u8; SIMPLE_FRAMING_LENGTH_FIELD_SIZE]) -> i32 {
  i32::from_be_bytes(bytes)
}
