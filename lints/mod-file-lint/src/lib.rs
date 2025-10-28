#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;

use std::{
  collections::HashSet,
  convert::TryFrom,
  path::{Path, PathBuf},
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
  pub NO_MOD_RS,
  Warn,
  "forbid legacy mod.rs files",
  NoModRs::default()
}

#[derive(Default)]
pub struct NoModRs {
  flagged: HashSet<PathBuf>,
}

impl<'tcx> LateLintPass<'tcx> for NoModRs {
  fn check_item(&mut self, cx: &LateContext<'tcx>, item: &Item<'tcx>) {
    let sm = cx.tcx.sess.source_map();
    let Some(path) = file_path_from_span(sm, item.span) else {
      return;
    };

    if !is_mod_rs(&path) {
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

    cx.span_lint(NO_MOD_RS, span, |diag| {
      diag.primary_message("mod.rs は Cellex のコーディング規約で使用禁止です");
      diag.note(format!("対象ファイル: {}", display_path));
      diag.help("ファイルを `<module>.rs` にリネームし、必要なら同ディレクトリに `<module>/` を作成してサブモジュールを移動してください");
      diag.note(
        "AI向けアドバイス: 1. `git mv foo/mod.rs foo.rs` 2. 親モジュールの `mod foo;` はそのまま利用 3. サブモジュールは foo.rs 内で `mod sub;` として再宣言しましょう。",
      );
    });
  }
}

fn is_mod_rs(path: &Path) -> bool {
  if path.components().any(|component| component.as_os_str() == "target") {
    return false;
  }
  path.file_name().is_some_and(|name| name == "mod.rs")
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
