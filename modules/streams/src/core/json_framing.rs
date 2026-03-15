#[cfg(test)]
mod tests;

use alloc::{boxed::Box, vec::Vec};
use core::any::TypeId;

use super::{
  DynValue, FlowDefinition, FlowLogic, MatCombine, StageDefinition, StreamError, StreamNotUsed, SupervisionStrategy,
  downcast_value,
  graph::StreamGraph,
  shape::{Inlet, Outlet},
  stage::{StageKind, flow::Flow},
};

/// JSON object framing utilities.
///
/// Provides a factory method that produces a flow splitting byte
/// streams into complete JSON objects using bracket-counting.
///
/// Corresponds to Pekko's `JsonFraming.objectScanner`.
pub struct JsonFraming;

impl JsonFraming {
  /// Returns a flow that emits complete JSON objects from a byte stream.
  ///
  /// Each input `Vec<u8>` chunk is accumulated into an internal buffer.
  /// Complete JSON objects (delimited by `{`/`}` or `[`/`]` pairs) are
  /// emitted downstream as `Vec<u8>`.  String literals and escape
  /// sequences are handled correctly so that brackets inside strings
  /// do not affect the depth count.
  ///
  /// Objects exceeding `maximum_object_length` cause a
  /// [`StreamError::BufferOverflow`](super::StreamError::BufferOverflow).
  /// If the source completes with an incomplete object in the buffer,
  /// a [`StreamError::Failed`](super::StreamError::Failed) is returned.
  #[must_use]
  pub fn object_scanner(maximum_object_length: usize) -> Flow<Vec<u8>, Vec<u8>, StreamNotUsed> {
    let definition = json_framing_definition(maximum_object_length);
    let mut graph = StreamGraph::new();
    graph.push_stage(StageDefinition::Flow(definition));
    Flow::from_graph(graph, StreamNotUsed::new())
  }
}

fn json_framing_definition(maximum_object_length: usize) -> FlowDefinition {
  let inlet: Inlet<Vec<u8>> = Inlet::new();
  let outlet: Outlet<Vec<u8>> = Outlet::new();
  let logic = JsonFramingLogic { buffer: Vec::new(), maximum_object_length, source_done: false };
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

struct JsonFramingLogic {
  buffer:                Vec<u8>,
  maximum_object_length: usize,
  source_done:           bool,
}

impl JsonFramingLogic {
  fn scan_objects(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let mut results: Vec<DynValue> = Vec::new();

    while let Some(start) = self.buffer.iter().position(|&b| b == b'{' || b == b'[') {
      let Some(object_bytes) = self.try_extract_object(start)? else {
        break;
      };
      results.push(Box::new(object_bytes) as DynValue);
    }

    Ok(results)
  }

  fn try_extract_object(&mut self, start: usize) -> Result<Option<Vec<u8>>, StreamError> {
    let open_bracket = self.buffer[start];
    let close_bracket = if open_bracket == b'{' { b'}' } else { b']' };

    let mut depth: usize = 0;
    let mut in_string = false;
    let mut in_escape = false;
    let mut pos = start;

    while pos < self.buffer.len() {
      let byte = self.buffer[pos];

      if in_escape {
        in_escape = false;
        pos += 1;
        continue;
      }

      if in_string {
        match byte {
          | b'\\' => in_escape = true,
          | b'"' => in_string = false,
          | _ => {},
        }
        pos += 1;
        continue;
      }

      match byte {
        | b'"' => in_string = true,
        | b if b == open_bracket => depth += 1,
        | b if b == close_bracket => {
          depth -= 1;
          if depth == 0 {
            let end = pos + 1;
            let object_bytes = self.buffer[start..end].to_vec();
            self.buffer = self.buffer[end..].to_vec();

            if object_bytes.len() > self.maximum_object_length {
              return Err(StreamError::BufferOverflow);
            }

            return Ok(Some(object_bytes));
          }
        },
        | _ => {},
      }
      pos += 1;
    }

    // Incomplete object — check if buffer exceeds the limit
    let pending_len = self.buffer.len() - start;
    if pending_len > self.maximum_object_length {
      return Err(StreamError::BufferOverflow);
    }

    Ok(None)
  }
}

impl FlowLogic for JsonFramingLogic {
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let chunk = downcast_value::<Vec<u8>>(input)?;
    self.buffer.extend_from_slice(&chunk);
    self.scan_objects()
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    if self.buffer.iter().any(|&b| b == b'{' || b == b'[') {
      return Err(StreamError::Failed);
    }
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    Ok(Vec::new())
  }

  fn has_pending_output(&self) -> bool {
    false
  }
}
