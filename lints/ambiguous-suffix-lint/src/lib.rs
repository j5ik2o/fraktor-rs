#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::{
  collections::HashSet,
  path::{Path, PathBuf},
};

use rustc_hir::{
  AmbigArg, FieldDef, GenericParam, GenericParamKind, ImplItem, ImplItemKind, Item, ItemKind, Pat, PatKind, TraitFn,
  TraitItem, TraitItemKind, Ty, TyKind, Variant,
};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{source_map::SourceMap, FileName, RealFileName, Span};

dylint_linting::impl_late_lint! {
  pub AMBIGUOUS_SUFFIX,
  Warn,
  "detect ambiguous name suffixes that obscure responsibility",
  AmbiguousSuffix::default()
}

#[derive(Default)]
pub struct AmbiguousSuffix {
  flagged_files: HashSet<PathBuf>,
}

/// Forbidden suffixes, their snake_case spellings, and their recommended alternatives.
const FORBIDDEN_SUFFIXES: &[(&str, &str, &str)] = &[
  ("Manager", "manager", "Registry, Coordinator, Dispatcher, Controller"),
  ("Util", "util", "具体的な動詞を含む名前 (例: FormatHelper → DateFormatter)"),
  ("Facade", "facade", "Gateway, Adapter, Bridge"),
  ("Service", "service", "Executor, Scheduler, Evaluator, Repository, Policy"),
  ("Runtime", "runtime", "Executor, Scheduler, EventLoop, Environment"),
  ("Engine", "engine", "Executor, Evaluator, Processor, Pipeline"),
];

impl<'tcx> LateLintPass<'tcx> for AmbiguousSuffix {
  fn check_item(&mut self, cx: &LateContext<'tcx>, item: &Item<'tcx>) {
    if !should_check_span(cx, item.span) {
      return;
    }

    let Some(path) = file_path_from_span(cx.tcx.sess.source_map(), item.span) else {
      return;
    };
    self.check_file_name(cx, item, &path);
    self.check_item_name(cx, item);
  }

  fn check_trait_item(&mut self, cx: &LateContext<'tcx>, trait_item: &TraitItem<'tcx>) {
    if !should_check_span(cx, trait_item.span) {
      return;
    }

    let (kind_label, name_style) = match trait_item.kind {
      | TraitItemKind::Const(..) => ("associated const", NameStyle::Delimited),
      | TraitItemKind::Fn(..) => ("trait method", NameStyle::Delimited),
      | TraitItemKind::Type(..) => ("associated type", NameStyle::Camel),
    };

    check_ident(cx, trait_item.ident, kind_label, name_style);

    if let TraitItemKind::Fn(_, TraitFn::Required(param_idents)) = trait_item.kind {
      for param_ident in param_idents.iter().flatten() {
        check_ident(cx, *param_ident, "parameter", NameStyle::Delimited);
      }
    }
  }

  fn check_impl_item(&mut self, cx: &LateContext<'tcx>, impl_item: &ImplItem<'tcx>) {
    if !should_check_span(cx, impl_item.span) {
      return;
    }

    let (kind_label, name_style) = match impl_item.kind {
      | ImplItemKind::Const(..) => ("associated const", NameStyle::Delimited),
      | ImplItemKind::Fn(..) => ("method", NameStyle::Delimited),
      | ImplItemKind::Type(..) => ("associated type", NameStyle::Camel),
    };

    check_ident(cx, impl_item.ident, kind_label, name_style);
  }

  fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &FieldDef<'tcx>) {
    if !field.is_positional() && should_check_span(cx, field.span) {
      check_ident(cx, field.ident, "field", NameStyle::Delimited);
    }
  }

  fn check_variant(&mut self, cx: &LateContext<'tcx>, variant: &Variant<'tcx>) {
    if should_check_span(cx, variant.span) {
      check_ident(cx, variant.ident, "variant", NameStyle::Camel);
    }
  }

  fn check_pat(&mut self, cx: &LateContext<'tcx>, pat: &Pat<'tcx>) {
    if !should_check_span(cx, pat.span) {
      return;
    }

    if let PatKind::Binding(_, _, ident, _) = pat.kind {
      check_ident(cx, ident, "variable", NameStyle::Delimited);
    }
  }

  fn check_generic_param(&mut self, cx: &LateContext<'tcx>, param: &GenericParam<'tcx>) {
    if param.is_elided_lifetime() || !should_check_span(cx, param.span) {
      return;
    }

    let ident = param.name.ident();
    let (kind_label, name_style) = match param.kind {
      | GenericParamKind::Lifetime { .. } => ("lifetime", NameStyle::Delimited),
      | GenericParamKind::Type { .. } => ("generic parameter", NameStyle::Camel),
      | GenericParamKind::Const { .. } => ("const parameter", NameStyle::Delimited),
    };

    check_ident(cx, ident, kind_label, name_style);
  }

  fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &Ty<'tcx, AmbigArg>) {
    if !should_check_span(cx, ty.span) {
      return;
    }

    if let TyKind::FnPtr(fn_ptr) = &ty.kind {
      for param_ident in fn_ptr.param_idents.iter().flatten() {
        check_ident(cx, *param_ident, "function pointer parameter", NameStyle::Delimited);
      }
    }
  }
}

impl AmbiguousSuffix {
  fn check_file_name(&mut self, cx: &LateContext<'_>, item: &Item<'_>, path: &Path) {
    if !self.flagged_files.insert(path.to_path_buf()) {
      return;
    }

    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
      return;
    };

    for &(suffix, snake_suffix, alternatives) in FORBIDDEN_SUFFIXES {
      if snake_name_has_forbidden_suffix(stem, snake_suffix) {
        emit_warning(cx, item.span, stem, "file", suffix, alternatives);
        break;
      }
    }
  }

  fn check_item_name(&self, cx: &LateContext<'_>, item: &Item<'_>) {
    if let Some((ident, kind_label, name_style)) = item_name_check(&item.kind) {
      check_ident(cx, ident, kind_label, name_style);
    }
  }
}

#[derive(Clone, Copy)]
enum NameStyle {
  Camel,
  Delimited,
}

fn item_name_check(kind: &ItemKind<'_>) -> Option<(rustc_span::symbol::Ident, &'static str, NameStyle)> {
  match kind {
    | ItemKind::Static(_, ident, ..) => Some((*ident, "static", NameStyle::Delimited)),
    | ItemKind::Const(ident, ..) => Some((*ident, "const", NameStyle::Delimited)),
    | ItemKind::Fn { ident, .. } => Some((*ident, "function", NameStyle::Delimited)),
    | ItemKind::Macro(ident, ..) => Some((*ident, "macro", NameStyle::Delimited)),
    | ItemKind::Mod(ident, ..) => Some((*ident, "module", NameStyle::Delimited)),
    | ItemKind::TyAlias(ident, ..) => Some((*ident, "type alias", NameStyle::Camel)),
    | ItemKind::Enum(ident, ..) => Some((*ident, "enum", NameStyle::Camel)),
    | ItemKind::Struct(ident, ..) => Some((*ident, "struct", NameStyle::Camel)),
    | ItemKind::Union(ident, ..) => Some((*ident, "union", NameStyle::Camel)),
    | ItemKind::Trait(_, _, _, ident, ..) => Some((*ident, "trait", NameStyle::Camel)),
    | ItemKind::TraitAlias(_, ident, ..) => Some((*ident, "trait alias", NameStyle::Camel)),
    | ItemKind::ExternCrate(..)
    | ItemKind::Use(..)
    | ItemKind::ForeignMod { .. }
    | ItemKind::GlobalAsm { .. }
    | ItemKind::Impl(..) => None,
  }
}

fn check_ident(cx: &LateContext<'_>, ident: rustc_span::symbol::Ident, kind_label: &str, name_style: NameStyle) {
  let name = ident.name.as_str();
  if name == "_" {
    return;
  }

  for &(suffix, snake_suffix, alternatives) in FORBIDDEN_SUFFIXES {
    let has_forbidden_suffix = match name_style {
      | NameStyle::Camel => camel_name_has_forbidden_suffix(name, suffix),
      | NameStyle::Delimited => delimited_name_has_forbidden_suffix(name, snake_suffix),
    };

    if has_forbidden_suffix {
      emit_warning(cx, ident.span, name, kind_label, suffix, alternatives);
      break;
    }
  }
}

fn emit_warning(cx: &LateContext<'_>, span: Span, name: &str, kind_label: &str, suffix: &str, alternatives: &str) {
  cx.span_lint(AMBIGUOUS_SUFFIX, span, |diag| {
    diag.primary_message(format!(
      "`{}` ({}) は曖昧なサフィックス `{}` を含んでいます",
      name, kind_label, suffix
    ));
    diag.help(format!(
      "`{}` は責務の境界が不明確になりやすいサフィックスです。代替案: {}",
      suffix, alternatives
    ));
    diag.note(
      "判定基準: この名前だけで「何に依存してよいか」「責務を一文で説明できるか」を確認してください".to_string(),
    );
    diag.note(format!(
      "AI向けアドバイス: 1. `{}` の責務を一文で定義する 2. その責務に合った具体的な名前を選ぶ（代替案: {}） 3. 外部API/フレームワーク由来の名前であれば `#[allow(ambiguous_suffix::ambiguous_suffix)]` で明示的に許可する",
      name, alternatives
    ));
  });
}

fn should_ignore(path: &Path) -> bool {
  if path.extension().map(|ext| ext != "rs").unwrap_or(true) {
    return true;
  }

  if path.components().any(|component| component.as_os_str() == "target") {
    return true;
  }

  let mut components = path.components().peekable();
  while let Some(component) = components.next() {
    if component.as_os_str() == "tests" {
      if components.peek().is_some_and(|next| next.as_os_str() == "ui") {
        continue;
      }
      return true;
    }
  }

  if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
    if file_name == "tests.rs" || file_name.ends_with("_tests.rs") {
      return true;
    }
  }

  false
}

fn should_check_span(cx: &LateContext<'_>, span: Span) -> bool {
  if span.from_expansion() {
    return false;
  }

  let Some(path) = file_path_from_span(cx.tcx.sess.source_map(), span) else {
    return false;
  };

  !should_ignore(&path)
}

fn camel_name_has_forbidden_suffix(name: &str, suffix: &str) -> bool {
  name == suffix || name.ends_with(suffix)
}

fn delimited_name_has_forbidden_suffix(name: &str, suffix: &str) -> bool {
  let normalized = name.trim_start_matches('\'').to_ascii_lowercase();
  snake_name_has_forbidden_suffix(&normalized, suffix)
}

fn snake_name_has_forbidden_suffix(name: &str, suffix: &str) -> bool {
  name == suffix || name.strip_suffix(suffix).is_some_and(|prefix| prefix.ends_with('_'))
}

fn file_path_from_span(sm: &SourceMap, span: Span) -> Option<PathBuf> {
  match sm.span_to_filename(span) {
    | FileName::Real(RealFileName::LocalPath(path)) => Some(path.to_path_buf()),
    | _ => None,
  }
}
