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

  /// Returns a flow that splits a JSON array byte stream into individual elements.
  ///
  /// The input is expected to be a single JSON array (`[...]`). Each
  /// top-level element inside the array is emitted downstream as an
  /// individual `Vec<u8>`. Nested arrays and objects are emitted whole.
  /// String literals and escape sequences are handled correctly.
  ///
  /// Elements exceeding `maximum_element_length` cause a
  /// [`StreamError::BufferOverflow`](super::StreamError::BufferOverflow).
  #[must_use]
  pub fn array_scanner(maximum_element_length: usize) -> Flow<Vec<u8>, Vec<u8>, StreamNotUsed> {
    let definition = json_array_framing_definition(maximum_element_length);
    let mut graph = StreamGraph::new();
    graph.push_stage(StageDefinition::Flow(definition));
    Flow::from_graph(graph, StreamNotUsed::new())
  }
}

fn json_framing_definition(maximum_object_length: usize) -> FlowDefinition {
  let inlet: Inlet<Vec<u8>> = Inlet::new();
  let outlet: Outlet<Vec<u8>> = Outlet::new();
  let logic = JsonFramingLogic { buffer: Vec::new(), maximum_object_length };
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
}

impl JsonFramingLogic {
  fn scan_objects(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let mut results: Vec<DynValue> = Vec::new();
    let mut deferred_error: Option<StreamError> = None;

    while let Some(start) = self.buffer.iter().position(|&b| b == b'{' || b == b'[') {
      // Discard leading non-bracket data so it cannot accumulate without limit.
      if start > 0 {
        self.buffer = self.buffer[start..].to_vec();
      }

      match self.try_extract_object(0) {
        | Ok(Some(object_bytes)) => results.push(Box::new(object_bytes) as DynValue),
        | Ok(None) => break,
        | Err(e) => {
          deferred_error = Some(e);
          break;
        },
      }
    }

    if deferred_error.is_none() && self.buffer.len() > self.maximum_object_length {
      deferred_error = Some(StreamError::BufferOverflow);
    }

    match deferred_error {
      | Some(e) if results.is_empty() => Err(e),
      | _ => Ok(results),
    }
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

            if end - start > self.maximum_object_length {
              return Err(StreamError::BufferOverflow);
            }

            let object_bytes = self.buffer[start..end].to_vec();
            self.buffer = self.buffer[end..].to_vec();
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

fn json_array_framing_definition(maximum_element_length: usize) -> FlowDefinition {
  let inlet: Inlet<Vec<u8>> = Inlet::new();
  let outlet: Outlet<Vec<u8>> = Outlet::new();
  let logic = JsonArrayFramingLogic { buffer: Vec::new(), maximum_element_length, found_open: false };
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

struct JsonArrayFramingLogic {
  buffer:                 Vec<u8>,
  maximum_element_length: usize,
  found_open:             bool,
}

impl JsonArrayFramingLogic {
  fn scan_elements(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let mut results: Vec<DynValue> = Vec::new();

    // Skip to the opening '[' if not found yet
    if !self.found_open {
      if let Some(pos) = self.buffer.iter().position(|&b| b == b'[') {
        self.buffer = self.buffer[pos + 1..].to_vec();
        self.found_open = true;
      } else {
        if self.buffer.len() > self.maximum_element_length {
          return Err(StreamError::BufferOverflow);
        }
        return Ok(results);
      }
    }

    loop {
      // Skip leading whitespace and commas
      while !self.buffer.is_empty() {
        match self.buffer[0] {
          | b' ' | b'\t' | b'\n' | b'\r' | b',' => {
            self.buffer.remove(0);
          },
          | _ => break,
        }
      }

      if self.buffer.is_empty() {
        break;
      }

      // Check for closing bracket
      if self.buffer[0] == b']' {
        self.buffer = self.buffer[1..].to_vec();
        break;
      }

      // Try to extract one element
      match self.try_extract_element() {
        | Ok(Some(element)) => results.push(Box::new(element) as DynValue),
        | Ok(None) => break,
        | Err(e) => {
          if results.is_empty() {
            return Err(e);
          }
          break;
        },
      }
    }

    Ok(results)
  }

  fn try_extract_element(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
    if self.buffer.is_empty() {
      return Ok(None);
    }

    let first = self.buffer[0];

    // Structured value: object or array
    if first == b'{' || first == b'[' {
      return self.try_extract_structured(first);
    }

    // String value
    if first == b'"' {
      return self.try_extract_string();
    }

    // Primitive value (number, true, false, null)
    self.try_extract_primitive()
  }

  fn try_extract_structured(&mut self, open: u8) -> Result<Option<Vec<u8>>, StreamError> {
    let close = if open == b'{' { b'}' } else { b']' };
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut in_escape = false;
    let mut pos = 0;

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
        | b if b == open => depth += 1,
        | b if b == close => {
          depth -= 1;
          if depth == 0 {
            let end = pos + 1;
            if end > self.maximum_element_length {
              return Err(StreamError::BufferOverflow);
            }
            let element = self.buffer[..end].to_vec();
            self.buffer = self.buffer[end..].to_vec();
            return Ok(Some(element));
          }
        },
        | _ => {},
      }
      pos += 1;
    }

    if self.buffer.len() > self.maximum_element_length {
      return Err(StreamError::BufferOverflow);
    }

    Ok(None)
  }

  fn try_extract_string(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
    let mut in_escape = false;
    let mut pos = 1; // Skip opening quote

    while pos < self.buffer.len() {
      let byte = self.buffer[pos];
      if in_escape {
        in_escape = false;
        pos += 1;
        continue;
      }
      match byte {
        | b'\\' => in_escape = true,
        | b'"' => {
          let end = pos + 1;
          if end > self.maximum_element_length {
            return Err(StreamError::BufferOverflow);
          }
          let element = self.buffer[..end].to_vec();
          self.buffer = self.buffer[end..].to_vec();
          return Ok(Some(element));
        },
        | _ => {},
      }
      pos += 1;
    }

    if self.buffer.len() > self.maximum_element_length {
      return Err(StreamError::BufferOverflow);
    }

    Ok(None)
  }

  fn try_extract_primitive(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
    // Primitives end at comma, closing bracket, or whitespace
    for pos in 0..self.buffer.len() {
      match self.buffer[pos] {
        | b',' | b']' | b' ' | b'\t' | b'\n' | b'\r' => {
          if pos == 0 {
            return Ok(None);
          }
          let element = self.buffer[..pos].to_vec();
          self.buffer = self.buffer[pos..].to_vec();
          return Ok(Some(element));
        },
        | _ => {},
      }
    }

    if self.buffer.len() > self.maximum_element_length {
      return Err(StreamError::BufferOverflow);
    }

    Ok(None)
  }
}

impl FlowLogic for JsonArrayFramingLogic {
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let chunk = downcast_value::<Vec<u8>>(input)?;
    self.buffer.extend_from_slice(&chunk);
    self.scan_elements()
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    // If there's remaining data with an incomplete element, emit what we can
    if !self.buffer.is_empty() && self.found_open {
      // Try to extract any remaining primitive at end of stream
      // (primitives might not have a trailing delimiter)
      // If there's actual structured data left, it's incomplete
      let trimmed: Vec<u8> = self.buffer.iter().copied().filter(|b| !b.is_ascii_whitespace()).collect();
      if !trimmed.is_empty() && trimmed != b"]" {
        return Err(StreamError::Failed);
      }
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
