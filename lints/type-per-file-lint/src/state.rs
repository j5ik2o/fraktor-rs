use rustc_span::Span;

pub struct TypeRecord {
  name: String,
  kind: &'static str,
  span: Span,
}

impl TypeRecord {
  pub fn new(name: String, kind: &'static str, span: Span) -> Self {
    Self { name, kind, span }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn kind(&self) -> &'static str {
    self.kind
  }

  pub fn span(&self) -> Span {
    self.span
  }

  pub fn describe(&self) -> String {
    format!("{} ({})", self.name, self.kind)
  }
}
