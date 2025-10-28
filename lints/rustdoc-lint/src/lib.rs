#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;

use std::{
  collections::HashSet,
  convert::TryFrom,
  fs,
  path::{Path, PathBuf},
};

use rustc_hir::{ForeignItem, ImplItem, Item, TraitItem};
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
  pub RUSTDOC_LINT,
  Warn,
  "enforce English rustdoc comments and executable code fences",
  RustdocLint::default()
}

#[derive(Default)]
pub struct RustdocLint {
  processed: HashSet<PathBuf>,
}

impl<'tcx> LateLintPass<'tcx> for RustdocLint {
  fn check_item(&mut self, cx: &LateContext<'tcx>, item: &Item<'tcx>) {
    self.process_span(cx, item.span);
  }

  fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &TraitItem<'tcx>) {
    self.process_span(cx, item.span);
  }

  fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &ImplItem<'tcx>) {
    self.process_span(cx, item.span);
  }

  fn check_foreign_item(&mut self, cx: &LateContext<'tcx>, item: &ForeignItem<'tcx>) {
    self.process_span(cx, item.span);
  }
}

impl RustdocLint {
  fn process_span(&mut self, cx: &LateContext<'_>, span: Span) {
    let sm = cx.tcx.sess.source_map();
    let Some(path) = file_path_from_span(sm, span) else {
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

  let sm = cx.tcx.sess.source_map();
  let Some(source_file) = load_source_file(sm, path) else {
    return;
  };

  let line_starts = compute_line_starts(&source);
  let lines: Vec<&str> = source.lines().collect();
  let blocks = extract_doc_blocks(&lines);

  for block in blocks {
    analyze_doc_block(cx, &block, &source_file, &line_starts);
  }
}

fn analyze_doc_block(
  cx: &LateContext<'_>,
  block: &DocBlock,
  source_file: &SourceFile,
  line_starts: &[usize],
) {
  let mut fence_state = FenceState::Outside;
  let mut emitted_non_english = false;

  for segment in &block.segments {
    let trimmed = segment.text.trim_start();

    if let Some(rest) = trimmed.strip_prefix("```") {
      match fence_state {
        | FenceState::Outside => {
          if fence_has_ignore(rest) {
            if let Some(span) = segment_span(segment, source_file, line_starts) {
              cx.span_lint(RUSTDOC_LINT, span, |diag| {
                diag.primary_message("rustdoc のコードフェンスでは `ignore` を使用できません");
                diag.help("サンプルコードは ` ```rust ` としてコンパイル可能な形で記述してください");
                diag.note("AI向けアドバイス: 実行不能なコードは避け、最小限の `rust` サンプルを提示しましょう。");
              });
            }
          }
          fence_state = FenceState::Inside;
        },
        | FenceState::Inside => {
          fence_state = FenceState::Outside;
        },
      }
      continue;
    }

    if matches!(fence_state, FenceState::Outside) && !emitted_non_english {
      if let Some((offset, ch_len)) = first_japanese_offset(&segment.text) {
        if let Some(span) = segment_char_span(segment, source_file, line_starts, offset, ch_len) {
          cx.span_lint(RUSTDOC_LINT, span, |diag| {
            diag.primary_message("rustdoc は英語で記述してください");
            diag.help("日本語などの全角文字はドキュメントコメントでは使わず、別資料に移動してください");
            diag.note("AI向けアドバイス: ドキュメント生成時には英語の説明文と `rust` のコード例を組み合わせて出力してください。");
          });
        }
        emitted_non_english = true;
      }
    }
  }
}

enum FenceState {
  Outside,
  Inside,
}

struct DocBlock {
  segments: Vec<DocSegment>,
}

struct DocSegment {
  text: String,
  line_idx: usize,
  content_column: usize,
  line_len: usize,
}

fn extract_doc_blocks(lines: &[&str]) -> Vec<DocBlock> {
  let mut blocks = Vec::new();
  let mut idx = 0;

  while idx < lines.len() {
    let line = lines[idx];
    let trimmed = line.trim_start();

    if trimmed.starts_with("///") {
      let (block, next_idx) = collect_line_doc_block(lines, idx, "///");
      blocks.push(block);
      idx = next_idx;
      continue;
    }

    if trimmed.starts_with("//!") {
      let (block, next_idx) = collect_line_doc_block(lines, idx, "//!");
      blocks.push(block);
      idx = next_idx;
      continue;
    }

    idx += 1;
  }

  blocks
}

fn collect_line_doc_block(lines: &[&str], start_idx: usize, marker: &str) -> (DocBlock, usize) {
  let mut segments = Vec::new();
  let mut idx = start_idx;

  while idx < lines.len() {
    let line = lines[idx];
    let trimmed = line.trim_start();
    if !trimmed.starts_with(marker) {
      break;
    }

    let indent = line.len() - trimmed.len();
    let mut content_column = indent + marker.len();
    if let Some(b' ') = line.as_bytes().get(content_column) {
      content_column += 1;
    }

    let content = if content_column <= line.len() {
      line[content_column.min(line.len())..].to_string()
    } else {
      String::new()
    };

    segments.push(DocSegment {
      text: content,
      line_idx: idx,
      content_column,
      line_len: line.len(),
    });

    idx += 1;
    if idx >= lines.len() {
      break;
    }

    let next_trimmed = lines[idx].trim_start();
    if !next_trimmed.starts_with(marker) {
      break;
    }
  }

  (DocBlock { segments }, idx)
}

fn fence_has_ignore(info: &str) -> bool {
  info
    .split(|ch: char| ch == ',' || ch.is_whitespace())
    .filter(|part| !part.is_empty())
    .any(|part| part.eq_ignore_ascii_case("ignore"))
}

fn first_japanese_offset(text: &str) -> Option<(usize, usize)> {
  for (idx, ch) in text.char_indices() {
    if is_japanese_char(ch) {
      return Some((idx, ch.len_utf8()));
    }
  }
  None
}

fn is_japanese_char(ch: char) -> bool {
  matches!(
    ch,
    '\u{3000}'
      | '\u{3001}'..='\u{303F}'
      | '\u{3040}'..='\u{309F}'
      | '\u{30A0}'..='\u{30FF}'
      | '\u{31F0}'..='\u{31FF}'
      | '\u{3400}'..='\u{4DBF}'
      | '\u{4E00}'..='\u{9FFF}'
      | '\u{F900}'..='\u{FAFF}'
      | '\u{FF66}'..='\u{FF9F}'
      | '\u{FFE0}'..='\u{FFEE}'
  )
}

fn segment_span(segment: &DocSegment, source_file: &SourceFile, line_starts: &[usize]) -> Option<Span> {
  let base = *line_starts.get(segment.line_idx)?;
  let start = base + segment.content_column;
  let end = base + segment.line_len;
  span_from_offsets(source_file, start, end)
}

fn segment_char_span(
  segment: &DocSegment,
  source_file: &SourceFile,
  line_starts: &[usize],
  offset: usize,
  len: usize,
) -> Option<Span> {
  let base = *line_starts.get(segment.line_idx)?;
  let start = base + segment.content_column + offset;
  let end = start + len;
  span_from_offsets(source_file, start, end)
}

fn span_from_offsets(source_file: &SourceFile, start: usize, end: usize) -> Option<Span> {
  if end < start {
    return None;
  }

  let lo = source_file.start_pos + BytePos(u32::try_from(start).ok()?);
  let hi = source_file.start_pos + BytePos(u32::try_from(end).ok()?);
  Some(Span::with_root_ctxt(lo, hi))
}

fn compute_line_starts(src: &str) -> Vec<usize> {
  let mut starts = vec![0];
  let mut offset = 0usize;
  for ch in src.chars() {
    offset += ch.len_utf8();
    if ch == '\n' {
      starts.push(offset);
    }
  }
  starts
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
