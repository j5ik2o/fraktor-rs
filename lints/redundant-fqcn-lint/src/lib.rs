#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;

use std::{
  collections::HashSet,
  convert::TryFrom,
  fs,
  path::{Path, PathBuf},
};

use proc_macro2::{LineColumn, Span as ProcSpan};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{source_map::SourceMap, BytePos, FileName, RealFileName, SourceFile, Span};
use syn::{
  spanned::Spanned,
  visit::{self, Visit},
  ExprPath, ExprStruct, File, Item, Pat, PatStruct, PatTupleStruct, Path as SynPath, UseTree,
};

dylint_linting::impl_late_lint! {
  pub REDUNDANT_FQCN,
  Warn,
  "detect redundant fully-qualified crate paths outside import declarations",
  RedundantFqcn::default()
}

#[derive(Default)]
pub struct RedundantFqcn {
  processed: HashSet<PathBuf>,
}

impl<'tcx> LateLintPass<'tcx> for RedundantFqcn {
  fn check_item(&mut self, cx: &LateContext<'tcx>, item: &rustc_hir::Item<'tcx>) {
    let sm = cx.tcx.sess.source_map();
    let Some(path) = file_path_from_span(sm, item.span) else {
      return;
    };

    if !self.processed.insert(path.clone()) {
      return;
    }

    analyze_file(cx, &path);
  }
}

fn analyze_file(cx: &LateContext<'_>, path: &Path) {
  let Ok(source) = fs::read_to_string(path) else {
    return;
  };

  let Ok(file) = syn::parse_file(&source) else {
    return;
  };

  let sm = cx.tcx.sess.source_map();
  let Some(source_file) = load_source_file(sm, path) else {
    return;
  };

  let line_starts = compute_line_starts(&source);
  let bindings = collect_use_bindings(&file);
  let mut collector = PathCollector::new(bindings);
  collector.visit_file(&file);

  for occurrence in collector.occurrences {
    if let Some(span) = span_for_item(&source_file, &line_starts, occurrence.span) {
      emit_warning(cx, span, &occurrence);
    }
  }
}

fn emit_warning(cx: &LateContext<'_>, span: Span, occurrence: &PathOccurrence) {
  cx.span_lint(REDUNDANT_FQCN, span, |diag| {
    diag.primary_message(format!(
      "`{}` は `use` 以外で不要な FQCN です",
      occurrence.display_path
    ));
    diag.help(format!(
      "`use {};` を追加し、本文では `{}` から始まる短い名前へ置き換えてください",
      occurrence.import_path, occurrence.short_path
    ));
    diag.note(format!(
      "AI向けアドバイス: 1. ファイル冒頭の `use` ブロックに `use {};` を追加する 2. この箇所の `{}` を `{}` から始まる短い表記に置き換える 3. 同じファイル内の同種の FQCN も同時に統一する 4. `use` 宣言以外の不要な変更は行わない",
      occurrence.import_path, occurrence.display_path, occurrence.short_path
    ));
  });
}

#[derive(Clone)]
struct PathOccurrence {
  span:         ProcSpan,
  display_path: String,
  import_path:  String,
  root_name:    String,
  short_path:   String,
}

struct PathCollector {
  occurrences: Vec<PathOccurrence>,
  seen:        HashSet<SpanKey>,
  bindings:    Vec<UseBinding>,
}

impl PathCollector {
  fn new(bindings: Vec<UseBinding>) -> Self {
    Self { occurrences: Vec::new(), seen: HashSet::new(), bindings }
  }

  fn record_path(&mut self, path: &SynPath) {
    let Some(occurrence) = build_occurrence(path) else {
      return;
    };
    if self.has_conflicting_import(&occurrence) {
      return;
    }
    let key = SpanKey::from_proc_span(occurrence.span);
    if self.seen.insert(key) {
      self.occurrences.push(occurrence);
    }
  }

  fn has_conflicting_import(&self, occurrence: &PathOccurrence) -> bool {
    self
      .bindings
      .iter()
      .any(|binding| binding.local_name == occurrence.root_name && binding.source_path != occurrence.import_path)
  }
}

impl<'ast> Visit<'ast> for PathCollector {
  fn visit_item(&mut self, item: &'ast Item) {
    match item {
      | Item::Use(_) | Item::Type(_) => {},
      | _ => visit::visit_item(self, item),
    }
  }

  fn visit_expr_path(&mut self, node: &'ast ExprPath) {
    if node.qself.is_none() {
      self.record_path(&node.path);
    }
    visit::visit_expr_path(self, node);
  }

  fn visit_expr_struct(&mut self, node: &'ast ExprStruct) {
    if node.qself.is_none() {
      self.record_path(&node.path);
    }
    visit::visit_expr_struct(self, node);
  }

  fn visit_pat(&mut self, node: &'ast Pat) {
    if let Pat::Path(path) = node {
      self.record_path(&path.path);
    }
    visit::visit_pat(self, node);
  }

  fn visit_pat_struct(&mut self, node: &'ast PatStruct) {
    if node.qself.is_none() {
      self.record_path(&node.path);
    }
    visit::visit_pat_struct(self, node);
  }

  fn visit_pat_tuple_struct(&mut self, node: &'ast PatTupleStruct) {
    if node.qself.is_none() {
      self.record_path(&node.path);
    }
    visit::visit_pat_tuple_struct(self, node);
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SpanKey {
  start_line: usize,
  start_col:  usize,
  end_line:   usize,
  end_col:    usize,
}

impl SpanKey {
  fn from_proc_span(span: ProcSpan) -> Self {
    let start = span.start();
    let end = span.end();
    Self { start_line: start.line, start_col: start.column, end_line: end.line, end_col: end.column }
  }
}

fn build_occurrence(path: &SynPath) -> Option<PathOccurrence> {
  if path.leading_colon.is_some() {
    return None;
  }

  let segments = path.segments.iter().collect::<Vec<_>>();
  let first = segments.first()?;
  let first_name = first.ident.to_string();
  if !is_supported_root(&first_name) {
    return None;
  }

  let type_like_index = segments
    .iter()
    .enumerate()
    .skip(usize::from(first_name == "crate" || first_name == "self" || first_name == "super"))
    .find_map(|(index, segment)| is_type_like_ident(segment.ident.to_string().as_str()).then_some(index))?;
  if type_like_index == 0 {
    return None;
  }

  let display_path = join_segment_idents(&segments);
  let import_path = join_segment_idents(&segments[..=type_like_index]);
  let root_name = segments.get(type_like_index)?.ident.to_string();
  let short_path = join_segment_idents(&segments[type_like_index..]);

  Some(PathOccurrence { span: path.span(), display_path, import_path, root_name, short_path })
}

fn is_type_like_ident(name: &str) -> bool {
  name.chars().next().is_some_and(|ch| ch.is_ascii_uppercase())
}

fn is_supported_root(name: &str) -> bool {
  matches!(name, "crate" | "self" | "super") || name.starts_with("fraktor_")
}

fn join_segment_idents(segments: &[&syn::PathSegment]) -> String {
  segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>().join("::")
}

#[derive(Clone)]
struct UseBinding {
  local_name:  String,
  source_path: String,
}

fn collect_use_bindings(file: &File) -> Vec<UseBinding> {
  let mut bindings = Vec::new();
  for item in &file.items {
    if let Item::Use(item_use) = item {
      collect_use_tree(&item_use.tree, String::new(), &mut bindings);
    }
  }
  bindings
}

fn collect_use_tree(tree: &UseTree, prefix: String, bindings: &mut Vec<UseBinding>) {
  match tree {
    | UseTree::Path(path) => {
      let next_prefix = append_path_segment(&prefix, path.ident.to_string().as_str());
      collect_use_tree(&path.tree, next_prefix, bindings);
    },
    | UseTree::Name(name) => {
      bindings.push(UseBinding {
        local_name:  name.ident.to_string(),
        source_path: append_path_segment(&prefix, name.ident.to_string().as_str()),
      });
    },
    | UseTree::Rename(rename) => {
      bindings.push(UseBinding {
        local_name:  rename.rename.to_string(),
        source_path: append_path_segment(&prefix, rename.ident.to_string().as_str()),
      });
    },
    | UseTree::Group(group) => {
      for item in &group.items {
        collect_use_tree(item, prefix.clone(), bindings);
      }
    },
    | UseTree::Glob(_) => {},
  }
}

fn append_path_segment(prefix: &str, segment: &str) -> String {
  if prefix.is_empty() {
    segment.to_string()
  } else {
    format!("{prefix}::{segment}")
  }
}

fn span_for_item(source_file: &SourceFile, line_starts: &[usize], span: ProcSpan) -> Option<Span> {
  let start = span.start();
  let end = span.end();
  let lo_offset = line_col_to_offset(line_starts, start)?;
  let hi_offset = line_col_to_offset(line_starts, end)?;
  let lo = source_file.start_pos + BytePos(u32::try_from(lo_offset).ok()?);
  let hi = source_file.start_pos + BytePos(u32::try_from(hi_offset).ok()?);
  Some(Span::with_root_ctxt(lo, hi))
}

fn compute_line_starts(src: &str) -> Vec<usize> {
  let mut starts = vec![0];
  let mut offset = 0usize;
  for ch in src.chars() {
    let next = offset + ch.len_utf8();
    if ch == '\n' {
      starts.push(next);
    }
    offset = next;
  }
  starts
}

fn line_col_to_offset(line_starts: &[usize], lc: LineColumn) -> Option<usize> {
  let line_idx = lc.line.checked_sub(1)? as usize;
  let base = *line_starts.get(line_idx)?;
  Some(base + lc.column as usize)
}

fn file_path_from_span(sm: &SourceMap, span: Span) -> Option<PathBuf> {
  match sm.span_to_filename(span) {
    | FileName::Real(RealFileName::LocalPath(path)) => Some(path.to_path_buf()),
    | _ => None,
  }
}

fn load_source_file(sm: &SourceMap, path: &Path) -> Option<std::sync::Arc<SourceFile>> {
  let filename = FileName::Real(RealFileName::LocalPath(path.to_path_buf()));
  sm.get_source_file(&filename).or_else(|| sm.load_file(path).ok())
}
