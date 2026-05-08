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
use syn::{spanned::Spanned, visit::Visit, File, Item};

dylint_linting::impl_late_lint! {
  pub USE_MOD_PLACEMENT,
  Deny,
  "require use/mod declarations to be grouped at the top of the file and forbid block-local use imports",
  UseModPlacement::default()
}

#[derive(Default)]
pub struct UseModPlacement {
  processed: HashSet<PathBuf>,
}

impl<'tcx> LateLintPass<'tcx> for UseModPlacement {
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
  enforce_layout(cx, &file, &source_file, &line_starts);
}

fn enforce_layout(cx: &LateContext<'_>, file: &File, source_file: &SourceFile, line_starts: &[usize]) {
  enforce_module_items(cx, &file.items, source_file, line_starts);
}

fn enforce_module_items(cx: &LateContext<'_>, items: &[Item], source_file: &SourceFile, line_starts: &[usize]) {
  let mut seen_non_top = false;

  for item in items {
    match classify_item(item) {
      | ItemClass::Use | ItemClass::Mod => {
        if seen_non_top {
          if let Some(span) = span_for_item(source_file, line_starts, item.span()) {
            emit_late_item(cx, span);
          }
        }
      },
      | ItemClass::Ignored => {},
      | ItemClass::Other => {
        seen_non_top = true;
        detect_block_local_uses(cx, item, source_file, line_starts);
      },
    }

    if let Item::Mod(module) = item {
      if let Some((_, nested_items)) = &module.content {
        enforce_module_items(cx, nested_items, source_file, line_starts);
      }
    }
  }
}

enum ItemClass {
  Use,
  Mod,
  Ignored,
  Other,
}

fn classify_item(item: &Item) -> ItemClass {
  match item {
    | Item::Use(_) => ItemClass::Use,
    | Item::Mod(module) => {
      if module.content.is_some() {
        ItemClass::Ignored
      } else {
        ItemClass::Mod
      }
    },
    | Item::ExternCrate(_) => ItemClass::Ignored,
    | Item::Type(_) => ItemClass::Ignored,
    | _ => ItemClass::Other,
  }
}

fn detect_block_local_uses(cx: &LateContext<'_>, item: &Item, source_file: &SourceFile, line_starts: &[usize]) {
  let mut visitor = BlockLocalUseVisitor { cx, source_file, line_starts };
  visitor.visit_item(item);
}

struct BlockLocalUseVisitor<'a, 'cx> {
  cx:          &'a LateContext<'cx>,
  source_file: &'a SourceFile,
  line_starts: &'a [usize],
}

impl<'ast> Visit<'ast> for BlockLocalUseVisitor<'_, '_> {
  fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
    if let Some(span) = span_for_item(self.source_file, self.line_starts, item.span()) {
      emit_block_local_use(self.cx, span);
    }
  }
}

fn emit_late_item(cx: &LateContext<'_>, span: Span) {
  cx.span_lint(USE_MOD_PLACEMENT, span, |diag| {
    diag.primary_message("`use` と `mod` 宣言はファイルまたはモジュール冒頭にまとめて配置してください");
    diag.help("この宣言を最初の `use` / `mod` ブロックより前に移動しましょう");
    diag.note("AI向けアドバイス: コメントや属性を含めたまま、該当の `use` / `mod` ブロックを冒頭へ移動して一箇所に集約してください。");
  });
}

fn emit_block_local_use(cx: &LateContext<'_>, span: Span) {
  cx.span_lint(USE_MOD_PLACEMENT, span, |diag| {
    diag.primary_message("関数やブロック内での `use` インポートは禁止です");
    diag.help("この `use` を、現在のファイルまたは enclosing module の冒頭 `use` ブロックへ移動してください");
    diag.note("AI向けアドバイス: 修正対象は該当 `use` と、その移動で必要になる参照パス調整だけです。周辺コードのリファクタリングは行わないでください。");
    diag.note("理由: 依存関係をファイル冒頭で一覧できるようにし、関数内に隠れたインポートを防ぐためです。");
  });
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
