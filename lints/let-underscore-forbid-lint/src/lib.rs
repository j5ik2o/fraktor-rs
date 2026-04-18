#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::path::{Path, PathBuf};

use rustc_hir::{ExprKind, LetStmt, PatKind, Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{FileName, RealFileName, Span, source_map::SourceMap, sym};

dylint_linting::impl_late_lint! {
  pub LET_UNDERSCORE_FORBID,
  Warn,
  "detect return-value discards that bypass `unused_must_use` / `clippy::let_underscore_must_use`",
  LetUnderscoreForbid
}

pub struct LetUnderscoreForbid;

/// Prefix that a caller must place on the immediately-preceding line to opt out
/// of the lint. The prefix must be followed by a non-empty reason.
///
/// 例: `// must-ignore: oneshot sender drop is best-effort on shutdown`
const ALLOW_PREFIX: &str = "// must-ignore:";

impl<'tcx> LateLintPass<'tcx> for LetUnderscoreForbid {
  fn check_local(&mut self, cx: &LateContext<'tcx>, local: &LetStmt<'tcx>) {
    // D1: `let _ = <expr>;` 形式の Local ノードすべてを検出。
    if local.span.from_expansion() {
      return;
    }
    if !matches!(local.pat.kind, PatKind::Wild) {
      return;
    }
    // `let _ : Ty;` のような初期化なし宣言は let 束縛として意味がないが、
    // 既存プロジェクトで使用されていないため念のため弾いておく。
    if local.init.is_none() {
      return;
    }

    let sm = cx.tcx.sess.source_map();
    let Some(path) = file_path_from_span(sm, local.span) else {
      return;
    };
    if should_ignore(&path) {
      return;
    }
    if has_must_ignore_comment(sm, local.span) {
      return;
    }

    emit_let_underscore(cx, local.span);
  }

  fn check_stmt(&mut self, cx: &LateContext<'tcx>, stmt: &Stmt<'tcx>) {
    // D2: `<expr>.ok();` の式文 (レシーバが Result)。
    // typeck_results を触る前に cheap な from_expansion() で弾いてコストを抑える。
    if stmt.span.from_expansion() {
      return;
    }
    let StmtKind::Semi(expr) = stmt.kind else {
      return;
    };
    let ExprKind::MethodCall(path_segment, receiver, args, _call_span) = expr.kind else {
      return;
    };
    if path_segment.ident.name.as_str() != "ok" || !args.is_empty() {
      return;
    }

    // レシーバが `Result<_, _>` であることを確認し、`Option::ok_or(_)` 等の
    // 別の "ok" メソッドや、自作 API の `ok()` を誤検出しないようにする。
    let ty = cx.typeck_results().expr_ty_adjusted(receiver);
    if !is_result_ty(cx, ty) {
      return;
    }

    let sm = cx.tcx.sess.source_map();
    let Some(path) = file_path_from_span(sm, stmt.span) else {
      return;
    };
    if should_ignore(&path) {
      return;
    }
    if has_must_ignore_comment(sm, stmt.span) {
      return;
    }

    emit_ok_discard(cx, stmt.span);
  }
}

fn emit_let_underscore(cx: &LateContext<'_>, span: Span) {
  cx.span_lint(LET_UNDERSCORE_FORBID, span, |diag| {
    diag.primary_message("`let _ = ...;` による戻り値の握りつぶしは禁止です");
    diag.note(
      ".agents/rules/ignored-return-values.md の MUST NOT に違反します。`Result` は `?` / \
       `match` / `if let Err(...)` で扱い、`Option` は明示的に unwrap / 分岐し、\
       `#[must_use]` 戻り値は値を受け取って評価してください。",
    );
    diag.help(format!(
      "例外を許容する場合は、違反行の直前行（空行を挟まない）に `{ALLOW_PREFIX} <理由>` を付与してください。\
       Drop / shutdown best-effort / メトリクス補助 / `Vec::pop` 相当 / 低レベル所有権操作のみ例外として許容されます。"
    ));
  });
}

fn emit_ok_discard(cx: &LateContext<'_>, span: Span) {
  cx.span_lint(LET_UNDERSCORE_FORBID, span, |diag| {
    diag.primary_message("`Result::ok();` でのエラー握りつぶしは禁止です");
    diag.note(
      ".agents/rules/ignored-return-values.md の MUST NOT に違反します。`.ok()` は `Result` から `Option` \
       への変換でエラー情報を捨てる意図を持つため、式文として値を捨てると失敗が観測不能になります。",
    );
    diag.help(format!(
      "失敗を伝播する (`?`)、ログ出力する (`if let Err(e) = ...`)、メトリクスに記録するなどで\
       失敗を可視化してください。例外を許容する場合は、違反行の直前行に `{ALLOW_PREFIX} <理由>` を付与してください。"
    ));
  });
}

/// 違反行の直前行に `// must-ignore: <reason>` が存在するか判定する。
///
/// 許容の条件:
/// - 直前行（空行を挟まない）が `// must-ignore:` で始まる
/// - プレフィックスの後に 1 文字以上の本文がある（理由の明示を強制する）
fn has_must_ignore_comment(sm: &SourceMap, span: Span) -> bool {
  let Ok(file_and_lines) = sm.span_to_lines(span) else {
    return false;
  };
  let Some(first_line) = file_and_lines.lines.first() else {
    return false;
  };
  let line_index = first_line.line_index; // 0-based
  if line_index == 0 {
    return false;
  }

  // 直前行の 0-based インデックスは `line_index - 1`。
  let prev_line_number = line_index - 1;
  let Some(prev_text) = file_and_lines.file.get_line(prev_line_number) else {
    return false;
  };

  let trimmed = prev_text.trim_start();
  let Some(reason) = trimmed.strip_prefix(ALLOW_PREFIX) else {
    return false;
  };
  !reason.trim().is_empty()
}

fn is_result_ty<'tcx>(cx: &LateContext<'tcx>, ty: rustc_middle::ty::Ty<'tcx>) -> bool {
  // 診断アイテム API を使うと pretty-print の変動 (`core::result::Result` 等)
  // に左右されず、`#[rustc_diagnostic_item = "Result"]` が付いた型を直接判定できる。
  // Clippy 本体と同じ方式。
  let ty = ty.peel_refs();
  let rustc_middle::ty::Adt(adt_def, _) = ty.kind() else {
    return false;
  };
  cx.tcx.get_diagnostic_item(sym::Result) == Some(adt_def.did())
}

fn should_ignore(path: &Path) -> bool {
  if path.extension().map(|ext| ext != "rs").unwrap_or(true) {
    return true;
  }
  if path.components().any(|component| component.as_os_str() == "target") {
    return true;
  }
  // `lints/*/tests/ui/` のフィクスチャ自身は検査対象外にする（ambiguous-suffix-lint と同様）。
  // ただし workspace 本体の `tests/` は検査対象に残す必要がある点に注意。
  // 4 連続コンポーネントで `lints / <crate> / tests / ui` を判定する。
  let components: Vec<_> = path.components().map(|c| c.as_os_str().to_owned()).collect();
  components.windows(4).any(|w| w[0] == "lints" && w[2] == "tests" && w[3] == "ui")
}

fn file_path_from_span(sm: &SourceMap, span: Span) -> Option<PathBuf> {
  match sm.span_to_filename(span) {
    | FileName::Real(RealFileName::LocalPath(path)) => Some(path.to_path_buf()),
    | _ => None,
  }
}
