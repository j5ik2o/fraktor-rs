#![feature(rustc_private)]

extern crate rustc_errors;

use rustc_errors::DiagDecorator;

extern crate rustc_hir;
extern crate rustc_span;

use std::{
  collections::{HashMap, HashSet},
  fs,
  path::{Path, PathBuf},
};

use proc_macro2::{LineColumn, Span as ProcSpan};
use rustc_hir::Item as HirItem;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{source_map::SourceMap, BytePos, SourceFile, Span};
use syn::{
  punctuated::Punctuated,
  spanned::Spanned,
  visit::{self, Visit},
  Attribute, Field, Fields, File as SynFile, Item, ItemStruct, Meta, MetaList, Path as SynPath, Token, Type, TypePath, UseTree,
  Visibility,
};

dylint_linting::impl_late_lint! {
  pub PORT_ADAPTOR_BOUNDARY,
  Warn,
  "detect public std adapter wrappers around core concrete API facades",
  PortAdaptorBoundary::default()
}

#[derive(Default)]
pub struct PortAdaptorBoundary {
  processed: HashSet<PathBuf>,
}

impl<'tcx> LateLintPass<'tcx> for PortAdaptorBoundary {
  fn check_item(&mut self, cx: &LateContext<'tcx>, item: &HirItem<'tcx>) {
    let sm = cx.tcx.sess.source_map();
    let Some(path) = file_path_from_span(sm, item.span) else {
      return;
    };

    if !is_adaptor_std_source(&path) || !self.processed.insert(path.clone()) {
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

  for violation in collect_violations(&file) {
    if let Some(rustc_span) = span_for_proc_span(&source_file, &line_starts, violation.span) {
      cx.emit_span_lint(PORT_ADAPTOR_BOUNDARY, rustc_span, DiagDecorator(|diag| {
        diag.primary_message(violation.primary_message());
        diag.help("修正手順: 1. core 側に必要最小の port trait を定義する 2. core concrete API はその port を実装する 3. std adapter は concrete API ではなく port trait に依存する");
        diag.note("コンテキスト: std adapter は core を wrap/drive せず、core が port 経由で adapter を駆動する構造にしてください。");
        diag.note("スコープ: 指摘された public struct と、その生成元/呼び出し元の依存型だけを変更してください。");
        diag.note("理由: DIP と Port & Adapter 境界を保ち、std が core concrete API の facade にならないようにするためです。");
      }));
    }
  }
}

fn collect_violations(file: &SynFile) -> Vec<Violation> {
  if file.attrs.iter().any(attr_allows_lint) {
    return Vec::new();
  }

  let imports = collect_core_imports(file);
  file
    .items
    .iter()
    .filter_map(|item| match item {
      | Item::Struct(item_struct) if is_public(&item_struct.vis) && !item_struct.attrs.iter().any(attr_allows_lint) => {
        Some(violations_for_struct(item_struct, &imports))
      },
      | _ => None,
    })
    .flatten()
    .collect()
}

fn violations_for_struct(item_struct: &ItemStruct, imports: &HashMap<String, CoreImport>) -> Vec<Violation> {
  let struct_name = item_struct.ident.to_string();
  let mut violations = Vec::new();

  match &item_struct.fields {
    | Fields::Named(fields) => {
      for field in &fields.named {
        let field_name = field.ident.as_ref().map(ToString::to_string);
        if let Some(violation) = violation_for_field(field, imports, &struct_name, field_name.as_deref()) {
          violations.push(violation);
        }
      }
    },
    | Fields::Unnamed(fields) => {
      for field in &fields.unnamed {
        if let Some(violation) = violation_for_field(field, imports, &struct_name, None) {
          violations.push(violation);
        }
      }
    },
    | Fields::Unit => {},
  }
  violations
}

fn violation_for_field(
  field: &Field,
  imports: &HashMap<String, CoreImport>,
  struct_name: &str,
  field_name: Option<&str>,
) -> Option<Violation> {
  if field.attrs.iter().any(attr_allows_lint) {
    return None;
  }

  core_concrete_usage(&field.ty, imports, struct_name, field_name).map(|usage| Violation { span: field.span(), usage })
}

fn core_concrete_usage(
  ty: &Type,
  imports: &HashMap<String, CoreImport>,
  struct_name: &str,
  field_name: Option<&str>,
) -> Option<CoreConcreteUsage> {
  struct TypeVisitor<'a> {
    imports:     &'a HashMap<String, CoreImport>,
    struct_name: &'a str,
    field_name:  Option<&'a str>,
    usage:       Option<CoreConcreteUsage>,
  }

  impl<'ast> Visit<'ast> for TypeVisitor<'_> {
    fn visit_type_path(&mut self, node: &'ast TypePath) {
      if self.usage.is_some() {
        return;
      }

      if let Some(usage) = classify_type_path(&node.path, self.imports, self.struct_name, self.field_name) {
        self.usage = Some(usage);
        return;
      }

      visit::visit_type_path(self, node);
    }
  }

  let mut visitor = TypeVisitor { imports, struct_name, field_name, usage: None };
  visitor.visit_type(ty);
  visitor.usage
}

fn classify_type_path(
  path: &SynPath,
  imports: &HashMap<String, CoreImport>,
  struct_name: &str,
  field_name: Option<&str>,
) -> Option<CoreConcreteUsage> {
  let last = path.segments.last()?.ident.to_string();

  if path_is_external_core(path) {
    if field_name == Some("inner") && last == struct_name {
      return Some(CoreConcreteUsage::SameNameWrapper { type_name: last });
    }
    if last.ends_with("Api") {
      return Some(CoreConcreteUsage::CoreApi { type_name: last });
    }
    return None;
  }

  if path.segments.len() != 1 {
    return None;
  }

  let import = imports.get(&last)?;
  let same_name_core_alias = field_name == Some("inner")
    && last == format!("Core{struct_name}")
    && (import.original == struct_name || import.original.trim_end_matches("Api") == struct_name);
  if same_name_core_alias {
    return Some(CoreConcreteUsage::SameNameWrapper { type_name: import.original.clone() });
  }

  if import.original.ends_with("Api") {
    return Some(CoreConcreteUsage::CoreApi { type_name: import.original.clone() });
  }

  None
}

fn collect_core_imports(file: &SynFile) -> HashMap<String, CoreImport> {
  let mut imports = HashMap::new();
  for item in &file.items {
    if let Item::Use(item_use) = item {
      collect_use_tree(&item_use.tree, &mut Vec::new(), &mut imports);
    }
  }
  imports
}

fn collect_use_tree(tree: &UseTree, prefix: &mut Vec<String>, imports: &mut HashMap<String, CoreImport>) {
  match tree {
    | UseTree::Path(path) => {
      prefix.push(path.ident.to_string());
      collect_use_tree(&path.tree, prefix, imports);
      prefix.pop();
    },
    | UseTree::Name(name) => {
      let mut full_path = prefix.clone();
      full_path.push(name.ident.to_string());
      if path_segments_are_external_core(&full_path) {
        imports.insert(name.ident.to_string(), CoreImport { original: name.ident.to_string() });
      }
    },
    | UseTree::Rename(rename) => {
      let mut full_path = prefix.clone();
      full_path.push(rename.ident.to_string());
      if path_segments_are_external_core(&full_path) {
        imports.insert(rename.rename.to_string(), CoreImport { original: rename.ident.to_string() });
      }
    },
    | UseTree::Group(group) => {
      for item in &group.items {
        collect_use_tree(item, prefix, imports);
      }
    },
    | UseTree::Glob(_) => {},
  }
}

fn path_is_external_core(path: &SynPath) -> bool {
  let segments: Vec<String> = path.segments.iter().map(|segment| segment.ident.to_string()).collect();
  path_segments_are_external_core(&segments)
}

fn path_segments_are_external_core(segments: &[String]) -> bool {
  segments.first().is_some_and(|root| root.starts_with("fraktor_") && root.contains("_core"))
}

fn is_public(vis: &Visibility) -> bool {
  matches!(vis, Visibility::Public(_))
}

fn attr_allows_lint(attr: &Attribute) -> bool {
  if !attr.path().is_ident("allow") {
    return false;
  }

  match &attr.meta {
    | Meta::List(list) => parse_meta_arguments(list).iter().any(|meta| match meta {
      | Meta::Path(path) => path.is_ident("port_adaptor_boundary"),
      | _ => false,
    }),
    | _ => false,
  }
}

fn parse_meta_arguments(list: &MetaList) -> Vec<Meta> {
  list
    .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
    .map(|punct| punct.into_iter().collect())
    .unwrap_or_default()
}

fn is_adaptor_std_source(path: &Path) -> bool {
  let normalized = path.to_string_lossy().replace('\\', "/");
  normalized.contains("/modules/") && normalized.contains("-adaptor-std/src/")
}

fn file_path_from_span(sm: &SourceMap, span: Span) -> Option<PathBuf> {
  sm.span_to_filename(span).into_local_path()
}

fn load_source_file(sm: &SourceMap, path: &Path) -> Option<std::sync::Arc<SourceFile>> {
  sm.load_file(path).ok()
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

fn span_for_proc_span(source_file: &SourceFile, line_starts: &[usize], span: ProcSpan) -> Option<Span> {
  let start = span.start();
  let end = span.end();
  let lo_offset = line_col_to_offset(line_starts, start)?;
  let hi_offset = line_col_to_offset(line_starts, end)?;
  let lo = source_file.start_pos + BytePos(u32::try_from(lo_offset).ok()?);
  let hi = source_file.start_pos + BytePos(u32::try_from(hi_offset).ok()?);
  Some(Span::with_root_ctxt(lo, hi))
}

fn line_col_to_offset(line_starts: &[usize], lc: LineColumn) -> Option<usize> {
  let line_idx = lc.line.checked_sub(1)? as usize;
  let base = *line_starts.get(line_idx)?;
  Some(base + lc.column as usize)
}

#[derive(Clone)]
struct CoreImport {
  original: String,
}

struct Violation {
  span:  ProcSpan,
  usage: CoreConcreteUsage,
}

impl Violation {
  fn primary_message(&self) -> String {
    match &self.usage {
      | CoreConcreteUsage::CoreApi { type_name } => {
        format!("std adapter の public struct が core concrete API `{type_name}` を保持しています")
      },
      | CoreConcreteUsage::SameNameWrapper { type_name } => {
        format!("std adapter の public struct が core concrete `{type_name}` を同名 wrapper として保持しています")
      },
    }
  }
}

enum CoreConcreteUsage {
  CoreApi { type_name: String },
  SameNameWrapper { type_name: String },
}
