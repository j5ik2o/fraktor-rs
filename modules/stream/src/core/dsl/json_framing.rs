#[cfg(test)]
mod tests;

use alloc::{boxed::Box, vec::Vec};
use core::any::TypeId;

use super::Flow;
use crate::core::{
  DynValue, FlowDefinition, FlowLogic, StageDefinition, StreamError, StreamNotUsed, SupervisionStrategy,
  attributes::Attributes,
  downcast_value,
  graph::StreamGraph,
  mat::MatCombine,
  shape::{Inlet, Outlet},
  stage::StageKind,
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
  /// [`StreamError::BufferOverflow`](crate::core::StreamError::BufferOverflow).
  /// If the source completes with an incomplete object in the buffer,
  /// a [`StreamError::Failed`](crate::core::StreamError::Failed) is returned.
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
  /// [`StreamError::BufferOverflow`](crate::core::StreamError::BufferOverflow).
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
    mat_combine: MatCombine::Left,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
    attributes:  Attributes::new(),
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

    if let Some(e) = deferred_error { Err(e) } else { Ok(results) }
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
  let logic = JsonArrayFramingLogic {
    buffer: Vec::new(),
    maximum_element_length,
    scan_offset: 0,
    state: JsonArrayState::AwaitingArrayStart,
  };
  FlowDefinition {
    kind:        StageKind::FlowStatefulMapConcat,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<Vec<u8>>(),
    output_type: TypeId::of::<Vec<u8>>(),
    mat_combine: MatCombine::Left,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
    attributes:  Attributes::new(),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsonArrayState {
  AwaitingArrayStart,
  AwaitingValueOrEnd,
  AwaitingValue,
  AwaitingSeparatorOrEnd,
  Closed,
}

struct JsonArrayFramingLogic {
  buffer:                 Vec<u8>,
  maximum_element_length: usize,
  scan_offset:            usize,
  state:                  JsonArrayState,
}

impl JsonArrayFramingLogic {
  fn current_byte(&self) -> Option<u8> {
    self.buffer.get(self.scan_offset).copied()
  }

  const fn remaining_len(&self) -> usize {
    self.buffer.len().saturating_sub(self.scan_offset)
  }

  fn skip_whitespace(&mut self) {
    while matches!(self.current_byte(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
      self.scan_offset += 1;
    }
  }

  fn compact_if_needed(&mut self) {
    if self.scan_offset == 0 {
      return;
    }
    if self.scan_offset < 1024 && self.scan_offset * 2 < self.buffer.len() {
      return;
    }
    self.buffer.drain(..self.scan_offset);
    self.scan_offset = 0;
  }

  fn scan_elements(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let mut results: Vec<DynValue> = Vec::new();

    loop {
      self.skip_whitespace();

      match self.state {
        | JsonArrayState::AwaitingArrayStart => {
          let Some(relative_pos) = self.buffer[self.scan_offset..].iter().position(|&byte| byte == b'[') else {
            if self.remaining_len() > self.maximum_element_length {
              return Err(StreamError::BufferOverflow);
            }
            break;
          };
          self.scan_offset += relative_pos + 1;
          self.state = JsonArrayState::AwaitingValueOrEnd;
          self.compact_if_needed();
        },
        | JsonArrayState::AwaitingValueOrEnd => {
          let Some(byte) = self.current_byte() else {
            break;
          };
          if byte == b']' {
            self.scan_offset += 1;
            self.state = JsonArrayState::Closed;
            self.compact_if_needed();
            break;
          }
          match self.try_extract_element() {
            | Ok(Some(element)) => {
              results.push(Box::new(element) as DynValue);
              self.state = JsonArrayState::AwaitingSeparatorOrEnd;
              self.compact_if_needed();
            },
            | Ok(None) => break,
            | Err(error) => return Err(error),
          }
        },
        | JsonArrayState::AwaitingValue => {
          let Some(byte) = self.current_byte() else {
            break;
          };
          if byte == b']' {
            return Err(StreamError::Failed);
          }
          match self.try_extract_element() {
            | Ok(Some(element)) => {
              results.push(Box::new(element) as DynValue);
              self.state = JsonArrayState::AwaitingSeparatorOrEnd;
              self.compact_if_needed();
            },
            | Ok(None) => break,
            | Err(error) => return Err(error),
          }
        },
        | JsonArrayState::AwaitingSeparatorOrEnd => {
          let Some(byte) = self.current_byte() else {
            break;
          };
          match byte {
            | b',' => {
              self.scan_offset += 1;
              self.state = JsonArrayState::AwaitingValue;
              self.compact_if_needed();
            },
            | b']' => {
              self.scan_offset += 1;
              self.state = JsonArrayState::Closed;
              self.compact_if_needed();
              break;
            },
            | _ => return Err(StreamError::Failed),
          }
        },
        | JsonArrayState::Closed => {
          if self.current_byte().is_some() {
            return Err(StreamError::Failed);
          }
          break;
        },
      }
    }

    Ok(results)
  }

  fn try_extract_element(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
    if self.scan_offset >= self.buffer.len() {
      return Ok(None);
    }

    let first = self.buffer[self.scan_offset];

    if first == b'{' || first == b'[' {
      return self.try_extract_structured(first);
    }

    if first == b'"' {
      return self.try_extract_string();
    }

    self.try_extract_primitive()
  }

  fn try_extract_structured(&mut self, open: u8) -> Result<Option<Vec<u8>>, StreamError> {
    let close = if open == b'{' { b'}' } else { b']' };
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut in_escape = false;
    let mut pos = self.scan_offset;

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
            if end - self.scan_offset > self.maximum_element_length {
              return Err(StreamError::BufferOverflow);
            }
            let element = self.buffer[self.scan_offset..end].to_vec();
            self.scan_offset = end;
            return Ok(Some(element));
          }
        },
        | _ => {},
      }
      pos += 1;
    }

    if self.remaining_len() > self.maximum_element_length {
      return Err(StreamError::BufferOverflow);
    }

    Ok(None)
  }

  fn try_extract_string(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
    let mut in_escape = false;
    let mut pos = self.scan_offset + 1;

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
          if end - self.scan_offset > self.maximum_element_length {
            return Err(StreamError::BufferOverflow);
          }
          let element = self.buffer[self.scan_offset..end].to_vec();
          self.scan_offset = end;
          return Ok(Some(element));
        },
        | _ => {},
      }
      pos += 1;
    }

    if self.remaining_len() > self.maximum_element_length {
      return Err(StreamError::BufferOverflow);
    }

    Ok(None)
  }

  fn try_extract_primitive(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
    if let Some(byte) = self.current_byte()
      && matches!(byte, b',' | b']')
    {
      return Err(StreamError::Failed);
    }

    for pos in self.scan_offset..self.buffer.len() {
      match self.buffer[pos] {
        | b',' | b']' | b' ' | b'\t' | b'\n' | b'\r' => {
          if pos == self.scan_offset {
            return Ok(None);
          }
          let element = self.buffer[self.scan_offset..pos].to_vec();
          self.scan_offset = pos;
          return Ok(Some(element));
        },
        | _ => {},
      }
    }

    if self.remaining_len() > self.maximum_element_length {
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
    self.skip_whitespace();
    match self.state {
      | JsonArrayState::AwaitingArrayStart => {
        if self.current_byte().is_some() {
          return Err(StreamError::Failed);
        }
        Ok(())
      },
      | JsonArrayState::Closed => {
        if self.current_byte().is_some() {
          return Err(StreamError::Failed);
        }
        Ok(())
      },
      | JsonArrayState::AwaitingValueOrEnd | JsonArrayState::AwaitingValue | JsonArrayState::AwaitingSeparatorOrEnd => {
        Err(StreamError::Failed)
      },
    }
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    Ok(Vec::new())
  }

  fn has_pending_output(&self) -> bool {
    false
  }
}
