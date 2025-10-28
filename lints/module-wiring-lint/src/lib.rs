#![feature(rustc_private)]

extern crate rustc_data_structures;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{
  self as hir,
  def_id::{DefId, LocalModDefId},
  Item,
  ItemKind,
  UseKind,
};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::Visibility;
use rustc_span::{symbol::Symbol, Span};

dylint_linting::impl_late_lint! {
  pub NO_PARENT_REEXPORT,
  Warn,
  "enforce module wiring re-export policy",
  NoParentReexport::default()
}

pub struct NoParentReexport {
  leaf_cache: FxHashMap<LocalModDefId, bool>,
}

impl Default for NoParentReexport {
  fn default() -> Self {
    Self { leaf_cache: FxHashMap::default() }
  }
}

impl<'tcx> LateLintPass<'tcx> for NoParentReexport {
  fn check_item(&mut self, cx: &LateContext<'tcx>, item: &Item<'tcx>) {
    match &item.kind {
      | ItemKind::Use(path, kind) => self.evaluate_use(cx, item, path, *kind),
      | ItemKind::Mod(ident, _) => self.evaluate_mod(cx, item, ident.name),
      | _ => {}
    }
  }
}

impl NoParentReexport {
  fn evaluate_mod<'tcx>(&mut self, cx: &LateContext<'tcx>, item: &Item<'tcx>, module_name: Symbol) {
    if has_allow_comment(cx, item.span) {
      return;
    }

    if module_name.as_str() == "prelude" {
      return;
    }

    // `mod foo { ... }` も `mod foo;` も対象とし、末端モジュールのみ判定する
    let module = LocalModDefId::new_unchecked(item.owner_id.def_id);
    if !self.is_leaf(cx, module) {
      return;
    }

    let visibility = cx.tcx.visibility(item.owner_id.def_id);
    let parent_scope = cx.tcx.parent_module_from_def_id(item.owner_id.def_id).to_def_id();

    let snippet = cx.tcx.sess.source_map().span_to_snippet(item.span);
    let vis_snippet = cx.tcx.sess.source_map().span_to_snippet(item.vis_span).ok();

    let explicit_visibility = vis_snippet.as_ref().map(|text| !text.trim().is_empty()).unwrap_or(false);

    let violates = match visibility {
      | Visibility::Public => true,
      | Visibility::Restricted(scope) => scope != parent_scope || explicit_visibility,
    };

    if !violates {
      return;
    }

    // `pub mod` / `pub(crate) mod` 等を弾く
    let detail = match snippet {
      | Ok(snippet) => format!("宣言 `{}` に公開可視性が設定されています", snippet.trim()),
      | Err(_) => format!("モジュール `{}` の宣言に公開可視性が設定されています", module_name.as_str()),
    };

    // inline module の場合でも `pub mod` は禁止
    self.emit_leaf_mod_visibility_violation(cx, item.span, &detail);
  }

  fn evaluate_use<'tcx>(&mut self, cx: &LateContext<'tcx>, item: &Item<'tcx>, path: &hir::UsePath<'tcx>, kind: UseKind) {
    if !is_public_use(cx, item) {
      return;
    }

    if has_allow_comment(cx, item.span) {
      return;
    }

    let current_mod = cx.tcx.parent_module_from_def_id(item.owner_id.def_id);

    if is_prelude_module(cx, current_mod.to_def_id()) {
      return;
    }

    let binding_ident = match kind {
      | UseKind::Single(ident) => Some(ident.name),
      | UseKind::Glob => None,
      | UseKind::ListStem => {
        return;
      },
    };

    let Some(first_segment) = path.segments.first() else {
      return;
    };

    if binding_ident.is_some() && path.segments.len() < 2 {
      self.emit_violation(cx, item.span, RuleLabel::Principle, "子モジュールを介さない再エクスポートは許可されていません", Some("末端モジュールにシンボルを配置し、親から `pub use child::Type;` として公開してください"));
      return;
    }

    if is_special_segment(first_segment.ident.name) {
      self.emit_violation(cx, item.span, RuleLabel::Principle, "特殊パス（self / super / crate）を経由した再エクスポートは禁止されています", Some("末端モジュールの直属親からのみ再エクスポートしてください"));
      return;
    }

    if let (Some(last_segment), Some(binding_name)) = (path.segments.last(), binding_ident) {
      if binding_name != last_segment.ident.name {
        self.emit_violation(
          cx,
          item.span,
          RuleLabel::Exception,
          "`as` を用いた再エクスポートは許可されていません",
          Some("末端モジュール内で元の名前を公開するか、呼び出し側で名前を付け替えてください"),
        );
        return;
      }
    }

    let child_name = first_segment.ident.name;

    let Some(child_mod) = self.find_direct_child_module(cx, current_mod, child_name) else {
      let detail = format!("モジュール `{}` はこのモジュールの直属の子として定義されていません", child_name.as_str());
      self.emit_violation(cx, item.span, RuleLabel::Principle, &detail, Some("`mod child;` を定義した親モジュール内でのみ `pub use child::Type;` を記述してください"));
      return;
    };

    if !self.is_leaf(cx, child_mod) {
      let detail = format!("モジュール `{}` にさらに子モジュールが存在するため末端モジュールではありません", child_name.as_str());
      self.emit_violation(
        cx,
        item.span,
        RuleLabel::Exception,
        &detail,
        Some("再エクスポートは葉モジュールの直属親でのみ行い、階層が深い場合は子モジュール側で公開してください"),
      );
      return;
    }
  }

  fn find_direct_child_module<'tcx>(&self, cx: &LateContext<'tcx>, parent: LocalModDefId, target: Symbol) -> Option<LocalModDefId> {
    let items = cx.tcx.hir_module_items(parent);
    for item_id in items.free_items() {
      let def_id = item_id.owner_id.def_id;
      let node = cx.tcx.hir_node_by_def_id(def_id);
      let item = node.expect_item();
      if let ItemKind::Mod(ident, _) = item.kind {
        if ident.name == target {
          return Some(LocalModDefId::new_unchecked(def_id));
        }
      }
    }
    None
  }

  fn is_leaf<'tcx>(&mut self, cx: &LateContext<'tcx>, module: LocalModDefId) -> bool {
    if let Some(&cached) = self.leaf_cache.get(&module) {
      return cached;
    }

    let items = cx.tcx.hir_module_items(module);
    let mut is_leaf = true;
    for item_id in items.free_items() {
      let def_id = item_id.owner_id.def_id;
      let node = cx.tcx.hir_node_by_def_id(def_id);
      let item = node.expect_item();
      if matches!(item.kind, ItemKind::Mod(_, _)) {
        is_leaf = false;
        break;
      }
    }

    self.leaf_cache.insert(module, is_leaf);
    is_leaf
  }

  fn emit_violation(&self, cx: &LateContext<'_>, span: Span, label: RuleLabel, detail: &str, help: Option<&'static str>) {
    cx.span_lint(NO_PARENT_REEXPORT, span, |diag| {
      diag.primary_message("再エクスポートは末端モジュールの直属親以外では禁止です");
      diag.note(label.message());
      diag.note(format!("詳細: {}", detail));
      if let Some(help_msg) = help {
        diag.help(help_msg);
      }
    });
  }

  fn emit_leaf_mod_visibility_violation(&self, cx: &LateContext<'_>, span: Span, detail: &str) {
    cx.span_lint(NO_PARENT_REEXPORT, span, |diag| {
      diag.primary_message("末端モジュールの宣言では `mod` のみを使用してください");
      diag.note("ルール: 末端モジュールを公開したい場合は親で `mod` のみ宣言し、`pub use` で公開します");
      diag.note(format!("詳細: {}", detail));
      diag.help("宣言から可視性修飾子を削除し、公開が必要なシンボルは末端モジュール内で `pub` を付けてください");
    });
  }
}

#[derive(Clone, Copy)]
enum RuleLabel {
  Principle,
  Exception,
}

impl RuleLabel {
  fn message(self) -> &'static str {
    match self {
      | RuleLabel::Principle => "ルール: 原則ルール: 再エクスポート禁止",
      | RuleLabel::Exception => "ルール: 例外ルール: 末端モジュールの直属親のみ許可",
    }
  }
}

fn is_special_segment(symbol: Symbol) -> bool {
  matches!(symbol.as_str(), "self" | "super" | "crate")
}

fn is_prelude_module(cx: &LateContext<'_>, def_id: DefId) -> bool {
  let path = cx.tcx.def_path_str(def_id);
  path.split("::").last() == Some("prelude")
}

fn has_allow_comment(cx: &LateContext<'_>, span: Span) -> bool {
  const TOKEN: &str = "allow module_wiring::no_parent_reexport";
  let sm = cx.tcx.sess.source_map();

  if let Ok(prev_source) = sm.span_to_prev_source(span) {
    if prev_source
      .lines()
      .last()
      .map(|line| line.trim_start().starts_with("//") && line.trim_start()[2..].trim_start().starts_with(TOKEN))
      .unwrap_or(false)
    {
      return true;
    }
  }

  let loc = sm.lookup_char_pos(span.lo());

  if comment_present(&loc.file, loc.line, Some(loc.col.0 as usize), TOKEN) {
    return true;
  }

  if loc.line > 0 && comment_present(&loc.file, loc.line - 1, None, TOKEN) {
    return true;
  }

  false
}

fn comment_present(file: &rustc_span::SourceFile, line_idx: usize, limit: Option<usize>, token: &str) -> bool {
  let Some(line) = file.get_line(line_idx) else {
    return false;
  };

  let text = line.as_ref();

  if scan_comment(text, token) {
    return true;
  }

  if let Some(end) = limit {
    let end = end.min(text.len());
    if end == 0 {
      return false;
    }
    return scan_comment(&text[..end], token);
  }

  false
}

fn scan_comment(segment: &str, token: &str) -> bool {
  if let Some(pos) = segment.rfind("//") {
    return segment[pos + 2..].trim().starts_with(token);
  }
  false
}


fn is_public_use(cx: &LateContext<'_>, item: &Item<'_>) -> bool {
  let sm = cx.tcx.sess.source_map();
  if let Ok(snippet) = sm.span_to_snippet(item.span) {
    if let Some(use_pos) = snippet.find("use") {
      let prefix = &snippet[..use_pos];
      if prefix.split_whitespace().any(|tok| tok.starts_with("pub")) {
        return true;
      }
      return false;
    }
    return false;
  }

  matches!(cx.tcx.visibility(item.owner_id.def_id), Visibility::Public | Visibility::Restricted(_))
}
