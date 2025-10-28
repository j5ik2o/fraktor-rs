#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;
extern crate rustc_middle;

use std::{
  collections::{hash_map::Entry, HashMap},
  path::{Path, PathBuf},
};

use heck::ToSnakeCase;

use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{source_map::SourceMap, FileName, RealFileName, Span};

mod state;

use state::TypeRecord;

dylint_linting::impl_late_lint! {
  pub MULTIPLE_TYPE_DEFINITIONS,
  Warn,
  "enforce single struct/enum/trait definition per file",
  MultipleTypeDefinitions::default()
}

pub struct MultipleTypeDefinitions {
  tracked: HashMap<PathBuf, Vec<TypeRecord>>,
}

impl Default for MultipleTypeDefinitions {
  fn default() -> Self {
    Self { tracked: HashMap::new() }
  }
}

impl<'tcx> LateLintPass<'tcx> for MultipleTypeDefinitions {
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

    let kind_label = describe_kind(&item.kind);
    let name = cx.tcx.item_name(def_id).to_string();

    match self.tracked.entry(path.clone()) {
      | Entry::Vacant(entry) => {
        entry.insert(vec![TypeRecord::new(name, kind_label, item.span)]);
      },
      | Entry::Occupied(mut entry) => {
        let records = entry.get_mut();

        let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("このファイル");
        let file_display = path.display().to_string();

        let first_detail = records
          .first()
          .map(|record| (record.span(), record.name().to_string(), record.kind()));

        let mut type_entries = Vec::with_capacity(records.len() + 1);
        for record in records.iter() {
          type_entries.push(record.describe());
        }
        type_entries.push(format!("{} ({})", name, kind_label));
        let type_list = type_entries.join(", ");

        let module_ident = name.to_snake_case();
        let suggested_name = format!("{}.rs", module_ident);
        let ai_note = format!(
          "AI向けアドバイス: 1. `{}` と同じディレクトリに `{}` を新規作成しこの `{}` を移動（サブディレクトリを掘らず `.rs` ファイル単体で配置） 2. 親モジュールで `mod {};` を追加 3. 必要なら `pub use` を調整して `cargo check` を実行してください。",
          file_name,
          suggested_name,
          name,
          module_ident
        );

        cx.span_lint(MULTIPLE_TYPE_DEFINITIONS, item.span, |diag| {
          diag.primary_message("構造体・列挙型・トレイトはファイルごとに1つまでにしてください");
          if let Some((span, existing_name, existing_kind)) = &first_detail {
            diag.span_note(
              *span,
              format!("このファイルでは既に `{}` ({}) が定義されています", existing_name, existing_kind)
            );
          }
          diag.note(format!("対象ファイル: {}", file_display));
          diag.note(format!("検出された型一覧: {}", type_list));
          diag.help("型ごとに別ファイルを作成し、親モジュールで `mod` 宣言を更新してください");
          diag.note(ai_note);
        });

        records.push(TypeRecord::new(name, kind_label, item.span));
      },
    }
  }
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
