#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;

use std::{
  collections::HashSet,
  convert::TryFrom,
  path::{Component, Path, PathBuf},
  sync::Arc,
};

use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{
  source_map::SourceMap,
  BytePos,
  FileName,
  RealFileName,
  SourceFile,
  Span,
};

dylint_linting::impl_late_lint! {
  pub MODULE_EXAMPLES_FORBID,
  Warn,
  "forbid runnable examples under modules/**/examples",
  ModuleExamplesForbid::default()
}

#[derive(Default)]
pub struct ModuleExamplesForbid {
  flagged: HashSet<PathBuf>,
}

impl<'tcx> LateLintPass<'tcx> for ModuleExamplesForbid {
  fn check_item(&mut self, cx: &LateContext<'tcx>, item: &Item<'tcx>) {
    let sm = cx.tcx.sess.source_map();
    let Some(path) = file_path_from_span(sm, item.span) else {
      return;
    };

    if !is_module_example_file(&path) {
      return;
    }

    if !self.flagged.insert(path.clone()) {
      return;
    }

    let Some(source_file) = load_source_file(sm, &path) else {
      return;
    };

    let span = first_line_span(&source_file);
    let display_path = path.display().to_string();

    cx.span_lint(MODULE_EXAMPLES_FORBID, span, |diag| {
      diag.primary_message("modules/**/examples 配下に runnable example を置いてはいけません");
      diag.note(format!("対象ファイル: {}", display_path));
      diag.help("example は `showcases/std/<example-name>/main.rs` へ移動してください");
      diag.note(
        "AI向けアドバイス: 1. 対象ファイルを `showcases/std` 配下へ移動 2. module crate の `Cargo.toml` から `[[example]]` を削除 3. `showcases/std/Cargo.toml` に example を登録しましょう。",
      );
    });
  }
}

fn is_module_example_file(path: &Path) -> bool {
  if path.extension().is_none_or(|extension| extension != "rs") {
    return false;
  }

  if path.components().any(|component| component.as_os_str() == "target") {
    return false;
  }

  let components: Vec<_> = path.components().collect();
  components.windows(4).any(|window| {
    matches!(
      window,
      [
        Component::Normal(first),
        Component::Normal(_crate_name),
        Component::Normal(examples),
        Component::Normal(_file_name),
      ] if *first == "modules" && *examples == "examples"
    )
  })
}

fn file_path_from_span(sm: &SourceMap, span: Span) -> Option<PathBuf> {
  match sm.span_to_filename(span) {
    | FileName::Real(RealFileName::LocalPath(path)) => Some(path.to_path_buf()),
    | _ => None,
  }
}

fn load_source_file(sm: &SourceMap, path: &Path) -> Option<Arc<SourceFile>> {
  let filename = FileName::Real(RealFileName::LocalPath(path.to_path_buf()));
  sm.get_source_file(&filename).or_else(|| sm.load_file(path).ok())
}

fn first_line_span(file: &SourceFile) -> Span {
  let lo = file.start_pos;
  let hi = if let Some(line) = file.get_line(0) {
    let text = line.as_ref();
    let len = u32::try_from(text.len()).ok().unwrap_or(0);
    let mut hi = lo + BytePos(len);
    if hi == lo {
      hi = lo + BytePos(1);
    }
    hi
  } else {
    lo + BytePos(1)
  };

  Span::with_root_ctxt(lo, hi)
}
