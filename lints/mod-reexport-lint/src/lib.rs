#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::{Item, ItemKind, UseKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::Visibility;

dylint_linting::impl_late_lint! {
  pub REDUNDANT_MOD_IMPORT,
  Warn,
  "avoid importing implementation detail modules directly",
  RedundantModImport::default()
}

#[derive(Default)]
pub struct RedundantModImport;

impl<'tcx> LateLintPass<'tcx> for RedundantModImport {
  fn check_item(&mut self, cx: &LateContext<'tcx>, item: &Item<'tcx>) {
    let ItemKind::Use(path, use_kind) = &item.kind else {
      return;
    };

    if matches!(cx.tcx.visibility(item.owner_id.to_def_id()), Visibility::Public) {
      return;
    }

    if let Ok(snippet) = cx.sess().source_map().span_to_snippet(item.span) {
      let trimmed = snippet.trim_start();
      if trimmed.starts_with("pub ") || trimmed.starts_with("pub(") {
        return;
      }
    }

    // Skip glob imports; they don't encode a specific item to re-export.
    if matches!(use_kind, UseKind::Glob) {
      return;
    }

    let segment_count = path.segments.len();
    if segment_count < 2 {
      return;
    }

    let res_opt =
      path.res.type_ns.and_then(|res| res.opt_def_id()).or_else(|| path.res.value_ns.and_then(|res| res.opt_def_id()));
    let Some(def_id) = res_opt else {
      return;
    };
    if !def_id.is_local() {
      return;
    }

    let last_segment = &path.segments[segment_count - 1];
    let parent_segment = &path.segments[segment_count - 2];

    if let Some(item_ident) = item.kind.ident() {
      if item_ident.name != last_segment.ident.name {
        return;
      }
    }

    let parent_name = parent_segment.ident.name.to_string();
    let expected = to_upper_camel(&parent_name);
    let actual = last_segment.ident.name.to_string();
    if expected != actual {
      return;
    }

    let module_path = path.segments[..segment_count - 1]
      .iter()
      .map(|segment| segment.ident.name.to_string())
      .collect::<Vec<_>>()
      .join("::");

    let span = last_segment.ident.span;

    cx.span_lint(REDUNDANT_MOD_IMPORT, span, |diag| {
      diag.primary_message(format!("`{}::{}` のような冗長な import です", module_path, actual));
      diag.help(format!(
        "`mod {};` と `pub use {}::{};` をこのファイルで宣言して再エクスポートしてください",
        parent_name,
        parent_name,
        actual
      ));
      diag.note(
        "AI向けアドバイス: 1. 同名モジュールを `mod` 宣言で読み込み 2. 公開が必要な型を `pub use` でまとめて再エクスポート 3. 呼び出し側の `use` を削除して親モジュールから直接利用しましょう。",
      );
    });
  }
}

fn to_upper_camel(ident: &str) -> String {
  ident
    .split('_')
    .filter(|segment| !segment.is_empty())
    .map(|segment| {
      let mut chars = segment.chars();
      match chars.next() {
        | Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        | None => String::new(),
      }
    })
    .collect()
}
