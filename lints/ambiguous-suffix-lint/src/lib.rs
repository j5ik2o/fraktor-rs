#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;
extern crate rustc_middle;

use std::path::{Path, PathBuf};

use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{source_map::SourceMap, FileName, RealFileName, Span};

dylint_linting::impl_late_lint! {
  pub AMBIGUOUS_SUFFIX,
  Warn,
  "detect ambiguous type name suffixes that obscure responsibility",
  AmbiguousSuffix
}

pub struct AmbiguousSuffix;

/// Forbidden suffixes and their recommended alternatives.
const FORBIDDEN_SUFFIXES: &[(&str, &str)] = &[
  ("Manager", "Registry, Coordinator, Dispatcher, Controller"),
  ("Util", "具体的な動詞を含む名前 (例: FormatHelper → DateFormatter)"),
  ("Facade", "Gateway, Adapter, Bridge"),
  ("Service", "Executor, Scheduler, Evaluator, Repository, Policy"),
  ("Runtime", "Executor, Scheduler, EventLoop, Environment"),
  ("Engine", "Executor, Evaluator, Processor, Pipeline"),
];

impl<'tcx> LateLintPass<'tcx> for AmbiguousSuffix {
  fn check_item(&mut self, cx: &LateContext<'tcx>, item: &Item<'tcx>) {
    if !matches!(item.kind, ItemKind::Struct(..) | ItemKind::Enum(..) | ItemKind::Trait(..)) {
      return;
    }

    if item.span.from_expansion() {
      return;
    }

    let sm = cx.tcx.sess.source_map();
    let Some(path) = file_path_from_span(sm, item.span) else {
      return;
    };

    if should_ignore(&path) {
      return;
    }

    let def_id = item.owner_id.def_id.to_def_id();
    if !cx.tcx.visibility(def_id).is_public() {
      return;
    }

    let name = cx.tcx.item_name(def_id).to_string();

    for &(suffix, alternatives) in FORBIDDEN_SUFFIXES {
      if name.ends_with(suffix) && name != suffix {
        let kind_label = describe_kind(&item.kind);
        emit_warning(cx, item.span, &name, kind_label, suffix, alternatives);
        break;
      }
    }
  }
}

fn emit_warning(
  cx: &LateContext<'_>,
  span: Span,
  name: &str,
  kind_label: &str,
  suffix: &str,
  alternatives: &str,
) {
  cx.span_lint(AMBIGUOUS_SUFFIX, span, |diag| {
    diag.primary_message(format!(
      "`{}` ({}) は曖昧なサフィックス `{}` を含んでいます",
      name, kind_label, suffix
    ));
    diag.help(format!(
      "`{}` は責務の境界が不明確になりやすいサフィックスです。代替案: {}",
      suffix, alternatives
    ));
    diag.note(format!(
      "判定基準: この名前だけで「何に依存してよいか」「責務を一文で説明できるか」を確認してください"
    ));
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

fn describe_kind(kind: &ItemKind<'_>) -> &'static str {
  match kind {
    | ItemKind::Struct(..) => "struct",
    | ItemKind::Enum(..) => "enum",
    | ItemKind::Trait(..) => "trait",
    | _ => "unknown",
  }
}

fn file_path_from_span(sm: &SourceMap, span: Span) -> Option<PathBuf> {
  match sm.span_to_filename(span) {
    | FileName::Real(RealFileName::LocalPath(path)) => Some(path.to_path_buf()),
    | _ => None,
  }
}
