#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;

use std::{
  collections::HashSet,
  fs,
  path::{Path, PathBuf},
};

use proc_macro2::{LineColumn, Span as ProcSpan};
use rustc_hir::Item as HirItem;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{source_map::SourceMap, BytePos, FileName, RealFileName, SourceFile, Span};
use syn::{
  punctuated::Punctuated,
  spanned::Spanned,
  visit::Visit,
  Attribute,
  Expr,
  File as SynFile,
  Item as SynItem,
  ItemUse,
  Lit,
  Meta,
  MetaList,
  Token,
  UseTree,
};

dylint_linting::impl_late_lint! {
  pub CFG_STD_FORBID,
  Allow,
  "detect #[cfg(feature = \"std\")] or std imports",
  CfgStdForbid::default()
}

#[derive(Default)]
pub struct CfgStdForbid {
  processed: HashSet<PathBuf>,
}

impl<'tcx> LateLintPass<'tcx> for CfgStdForbid {
  fn check_item(&mut self, cx: &LateContext<'tcx>, item: &HirItem<'tcx>) {
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

  let allow_entire_file =
    file.attrs.iter().any(attr_is_cfg_test) || file.attrs.iter().any(attr_allows_lint);
  let line_starts = compute_line_starts(&source);
  for violation in collect_forbidden_spans(&file, allow_entire_file) {
    match violation.kind {
      | ViolationKind::CfgStd(attr_span) => {
        if let Some(rustc_span) = span_for_attribute(&source_file, &line_starts, attr_span) {
          cx.span_lint(CFG_STD_FORBID, rustc_span, |diag| {
            diag.primary_message("`#[cfg(feature = \"std\")]` の使用が検出されました");
            diag.help("std 依存コードは std 対応クレートへ移動するか、必要な範囲で `#![allow(cfg_std_forbid)]` を付与してください");
            diag.note("AI向けアドバイス: std が不可欠なロジックは別モジュールへ切り離し、lint の適用境界を明確にしましょう。");
          });
        }
      },
      | ViolationKind::UseStd(use_span) => {
        if let Some(rustc_span) = span_for_attribute(&source_file, &line_starts, use_span) {
          cx.span_lint(CFG_STD_FORBID, rustc_span, |diag| {
            diag.primary_message("`use std::...` の使用が検出されました");
            diag.help("std 名前空間のアイテムは std 対応クレートへ移動するか、該当箇所で `#![allow(cfg_std_forbid)]` を付与してください");
            diag.note("AI向けアドバイス: core/embedded コードでは `std` 依存を避け、必要に応じて `alloc` など代替を検討しましょう。");
          });
        }
      },
    }
  }
}

fn collect_forbidden_spans(file: &SynFile, allow_initial: bool) -> Vec<Violation> {
  struct Visitor {
    spans: Vec<Violation>,
    allow_std: bool,
  }

  impl<'ast> Visit<'ast> for Visitor {
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
      if attr_allows_lint(attr) {
        self.allow_std = true;
        return;
      }

      if attr_is_cfg_test(attr) {
        let prev = self.allow_std;
        self.allow_std = true;
        syn::visit::visit_attribute(self, attr);
        self.allow_std = prev;
        return;
      }

      if !self.allow_std && is_forbidden_cfg(attr) {
        self.spans.push(Violation::new_cfg(attr.span()));
      }
      syn::visit::visit_attribute(self, attr);
    }

    fn visit_item(&mut self, i: &'ast SynItem) {
      let allow_for_item =
        self.allow_std || item_has_cfg_test(i) || item_allows_lint(i);
      let prev = self.allow_std;
      self.allow_std = allow_for_item;

      if !self.allow_std {
        if let SynItem::Use(item_use) = i {
          if use_tree_contains_std(&item_use) {
            self.spans.push(Violation::new_use(item_use.span()));
          }
        }
      }

      syn::visit::visit_item(self, i);
      self.allow_std = prev;
    }
  }

  let mut visitor = Visitor {
    spans: Vec::new(),
    allow_std: allow_initial,
  };
  visitor.visit_file(file);
  visitor.spans
}

fn item_has_cfg_test(item: &SynItem) -> bool {
  item_attrs(item).iter().any(attr_is_cfg_test)
}

fn item_allows_lint(item: &SynItem) -> bool {
  item_attrs(item).iter().any(attr_allows_lint)
}

fn item_attrs(item: &SynItem) -> &[Attribute] {
  match item {
    | SynItem::Const(item) => &item.attrs,
    | SynItem::Enum(item) => &item.attrs,
    | SynItem::ExternCrate(item) => &item.attrs,
    | SynItem::Fn(item) => &item.attrs,
    | SynItem::ForeignMod(item) => &item.attrs,
    | SynItem::Impl(item) => &item.attrs,
    | SynItem::Macro(item) => &item.attrs,
    | SynItem::Mod(item) => &item.attrs,
    | SynItem::Static(item) => &item.attrs,
    | SynItem::Struct(item) => &item.attrs,
    | SynItem::Trait(item) => &item.attrs,
    | SynItem::TraitAlias(item) => &item.attrs,
    | SynItem::Type(item) => &item.attrs,
    | SynItem::Union(item) => &item.attrs,
    | SynItem::Use(item) => &item.attrs,
    | SynItem::Verbatim(_) => &[],
    | _ => &[],
  }
}

fn attr_is_cfg_test(attr: &Attribute) -> bool {
  if !attr.path().is_ident("cfg") {
    return false;
  }

  match &attr.meta {
    | Meta::List(list) => {
      let args = parse_meta_arguments(&list);
      args.iter().any(meta_requires_test)
    },
    | _ => false,
  }
}

fn attr_allows_lint(attr: &Attribute) -> bool {
  if !attr.path().is_ident("allow") {
    return false;
  }

  match &attr.meta {
    | Meta::List(list) => parse_meta_arguments(list)
      .iter()
      .any(|meta| matches!(meta, Meta::Path(path) if path.is_ident("cfg_std_forbid"))),
    | _ => false,
  }
}

fn meta_requires_test(meta: &Meta) -> bool {
  match meta {
    | Meta::Path(path) => path.is_ident("test"),
    | Meta::List(list) if list.path.is_ident("all") => {
      let args = parse_meta_arguments(list);
      let has_test = args.iter().any(meta_requires_test);
      let has_forbidden = args.iter().any(meta_allows_without_test);
      has_test && !has_forbidden
    },
    | _ => false,
  }
}

fn meta_allows_without_test(meta: &Meta) -> bool {
  match meta {
    | Meta::List(list) if list.path.is_ident("not") || list.path.is_ident("any") => true,
    | Meta::List(list) => parse_meta_arguments(list).iter().any(meta_allows_without_test),
    | _ => false,
  }
}

fn is_forbidden_cfg(attr: &Attribute) -> bool {
  if !attr.path().is_ident("cfg") {
    return false;
  }

  match &attr.meta {
    | Meta::List(list) => {
      let items = parse_meta_arguments(&list);
      items.iter().any(contains_feature_std)
    },
    | _ => false,
  }
}

fn contains_feature_std(meta: &Meta) -> bool {
  match meta {
    | Meta::NameValue(name_value) => name_value.path.is_ident("feature") && expr_is_std(&name_value.value),
    | Meta::List(list) => contains_feature_std_list(list),
    | Meta::Path(_) => false,
  }
}

fn contains_feature_std_list(list: &MetaList) -> bool {
  let args = parse_meta_arguments(list);
  if list.path.is_ident("not") {
    return false;
  }

  let has_std = args.iter().any(|item| match item {
    | Meta::Path(path) => path.is_ident("std"),
    | Meta::NameValue(nv) => expr_is_std(&nv.value),
    | Meta::List(inner) => contains_feature_std_list(inner),
  });

  if list.path.is_ident("feature") {
    has_std
  } else {
    args.iter().any(contains_feature_std)
  }
}

fn expr_is_std(expr: &Expr) -> bool {
  match expr {
    | Expr::Lit(literal) => match &literal.lit {
      | Lit::Str(value) => value.value() == "std",
      | _ => false,
    },
    | _ => false,
  }
}

fn parse_meta_arguments(list: &MetaList) -> Vec<Meta> {
  list
    .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
    .map(|punct| punct.into_iter().collect())
    .unwrap_or_default()
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

fn span_for_attribute(source_file: &SourceFile, line_starts: &[usize], span: ProcSpan) -> Option<Span> {
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

fn use_tree_contains_std(item_use: &ItemUse) -> bool {
  fn tree_contains(tree: &UseTree) -> bool {
    match tree {
      | UseTree::Path(path) => {
        if path.ident == "std" {
          true
        } else {
          tree_contains(&path.tree)
        }
      },
      | UseTree::Name(name) => name.ident == "std",
      | UseTree::Group(group) => group.items.iter().any(tree_contains),
      | UseTree::Rename(rename) => rename.ident == "std",
      | UseTree::Glob(_) => false,
    }
  }

  tree_contains(&item_use.tree)
}

struct Violation {
  kind: ViolationKind,
}

impl Violation {
  fn new_cfg(span: ProcSpan) -> Self {
    Self {
      kind: ViolationKind::CfgStd(span),
    }
  }

  fn new_use(span: ProcSpan) -> Self {
    Self {
      kind: ViolationKind::UseStd(span),
    }
  }
}

enum ViolationKind {
  CfgStd(ProcSpan),
  UseStd(ProcSpan),
}
