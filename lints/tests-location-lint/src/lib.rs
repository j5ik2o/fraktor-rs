#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;

use std::{
  collections::HashSet,
  fs,
  ops::Range,
  path::{Path, PathBuf},
  sync::Arc,
};

use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{source_map::SourceMap, BytePos, FileName, RealFileName, SourceFile, Span};

dylint_linting::impl_late_lint! {
  pub SEPARATE_TESTS,
  Warn,
  "enforce moving tests into dedicated tests.rs files",
  SeparateTests::default()
}

#[derive(Default)]
pub struct SeparateTests {
  processed: HashSet<PathBuf>,
}

impl<'tcx> LateLintPass<'tcx> for SeparateTests {
  fn check_item(&mut self, cx: &LateContext<'tcx>, item: &Item<'tcx>) {
    let sm = cx.tcx.sess.source_map();
    let Some(path) = file_path_from_span(sm, item.span) else {
      return;
    };

    if !self.processed.insert(path.clone()) || is_tests_path(&path) {
      return;
    }

    analyze_source_file(cx, &path);
  }
}

fn analyze_source_file(cx: &LateContext<'_>, path: &Path) {
  let Ok(source) = fs::read_to_string(path) else {
    return;
  };

  let sm = cx.tcx.sess.source_map();
  let Some(source_file) = load_source_file(sm, path) else {
    return;
  };

  for range in find_inline_test_modules(&source) {
    if let Some(span) = span_for_range(&source_file, range) {
      cx.span_lint(SEPARATE_TESTS, span, |diag| {
        diag.primary_message("`mod tests { ... }` must reside in a sibling tests.rs file");
        diag.help("move the test module into `<module>/tests.rs` and keep only `#[cfg(test)] mod tests;`");
        diag.note("AI向けアドバイス: `<module>/tests.rs` を作成し、このインラインモジュールを丸ごと移動。親ファイルには `#[cfg(test)] mod tests;` だけを残し、モジュール階層が正しいか確認してください。");
      });
    }
  }

  for range in find_cfg_test_items(&source) {
    if let Some(span) = span_for_range(&source_file, range) {
      cx.span_lint(SEPARATE_TESTS, span, |diag| {
        diag.primary_message("`#[cfg(test)]` items are not allowed in production modules");
        diag.help("extract the item into `<module>/tests.rs` and gate it via `#[cfg(test)] mod tests;`");
        diag.note(
          "AI向けアドバイス: テスト専用コードは `<module>/tests.rs` に移動し、親には `#[cfg(test)] mod tests;` を追加。構造体メソッドなら tests 側で `impl 型名 { #[cfg(test)] fn ... }` を定義し、`use super::*;` で型やフィールドへアクセスしてください。"
        );
      });
    }
  }

  for range in find_direct_test_attributes(&source) {
    if let Some(span) = span_for_range(&source_file, range) {
      cx.span_lint(SEPARATE_TESTS, span, |diag| {
        diag.primary_message("test functions must be moved into a dedicated tests.rs file");
        diag.help("place this test under `<module>/tests.rs` ensuring the parent declares `#[cfg(test)] mod tests;`");
        diag.note("AI向けアドバイス: このテスト関数と依存するヘルパーを `<module>/tests.rs` に移動し、必要なら `use super::*;` などを追加してビルドが通るようにしてください。");
      });
    }
  }
}

fn file_path_from_span(sm: &SourceMap, span: Span) -> Option<PathBuf> {
  match sm.span_to_filename(span) {
    | FileName::Real(RealFileName::LocalPath(path)) => Some(path.to_path_buf()),
    | _ => None,
  }
}

fn is_tests_path(path: &Path) -> bool {
  if path.file_name().and_then(|s| s.to_str()) == Some("tests.rs") {
    return true;
  }
  path.components().any(|component| component.as_os_str() == "tests")
}

fn load_source_file(sm: &SourceMap, path: &Path) -> Option<Arc<SourceFile>> {
  let filename = FileName::Real(RealFileName::LocalPath(path.to_path_buf()));
  sm.get_source_file(&filename).or_else(|| sm.load_file(path).ok())
}

fn span_for_range(file: &SourceFile, range: Range<usize>) -> Option<Span> {
  let start = u32::try_from(range.start).ok()?;
  let end = u32::try_from(range.end).ok()?;
  let lo = file.start_pos + BytePos(start);
  let hi = file.start_pos + BytePos(end);
  Some(Span::with_root_ctxt(lo, hi))
}

fn find_inline_test_modules(src: &str) -> Vec<Range<usize>> {
  let mut matches = Vec::new();
  let mut cursor = 0;
  const PATTERN: &str = "mod tests";

  while let Some(offset) = src[cursor..].find(PATTERN) {
    let absolute = cursor + offset;
    let after = &src[absolute + PATTERN.len()..];
    let skipped = after.chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum::<usize>();
    let after_trimmed = &after[skipped..];
    if after_trimmed.starts_with('{') {
      let span_len = PATTERN.len() + skipped + 1;
      matches.push(absolute..absolute + span_len);
    }
    cursor = absolute + PATTERN.len();
  }

  matches
}

fn find_cfg_test_items(src: &str) -> Vec<Range<usize>> {
  let mut matches = Vec::new();
  const ATTR: &str = "#[cfg(test)]";
  let mut cursor = 0;

  while let Some(offset) = src[cursor..].find(ATTR) {
    let absolute = cursor + offset;
    let rest = &src[absolute + ATTR.len()..];
    let skipped = rest.chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum::<usize>();
    let after = &rest[skipped..];
    if after.starts_with("mod tests;") {
      cursor = absolute + ATTR.len();
      continue;
    }
    if after.starts_with("mod tests") {
      // `mod tests {` will be handled separately to avoid duplicate diagnostics.
      cursor = absolute + ATTR.len();
      continue;
    }
    matches.push(absolute..absolute + ATTR.len());
    cursor = absolute + ATTR.len();
  }

  matches
}

fn find_direct_test_attributes(src: &str) -> Vec<Range<usize>> {
  let mut matches = Vec::new();
  let mut cursor = 0;

  while let Some(start) = src[cursor..].find("#[") {
    let absolute = cursor + start;
    let remaining = &src[absolute + 2..];
    let Some(closing) = find_attribute_end(remaining) else {
      break;
    };
    let attr_body = &remaining[..closing];
    let attr_name = attr_body.trim().split(|c: char| c == '(' || c.is_whitespace()).next().unwrap_or("");

    if attr_name.ends_with("::test") || attr_name == "test" {
      matches.push(absolute..absolute + 2 + closing + 1);
    }

    cursor = absolute + 2 + closing + 1;
  }

  matches
}

fn find_attribute_end(segment: &str) -> Option<usize> {
  let mut depth = 0usize;
  for (idx, ch) in segment.char_indices() {
    match ch {
      | '[' => depth += 1,
      | ']' => {
        if depth == 0 {
          return Some(idx);
        }
        depth -= 1;
      },
      | _ => {},
    }
  }
  None
}
