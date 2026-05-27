{extends:review-qa}

## Fraktor/Pekko 固有の追加観点

- fraktor-rs はドメインごとに別クレートへ分離されている。コアクレート（`modules/*-core*`）は原則 `no_std` + Sans I/O、std アダプタクレート（`modules/*-adaptor-std/`）は std / tokio 依存とする。
- core クレートのプロダクトコードに std 依存の例外を追加してはならない。例外の導入は人間の明示指示がある場合のみ許可する。
- std/embedded 依存の無いポート定義（trait）が core クレート側に正しく配置されているか確認する。
- `showcases` の網羅性を利用者視点で確認する。`modules/*-core*/src/**/*.rs` や `modules/*-adaptor-std/src/**/*.rs` に対応する `showcases/std/**/*.rs` が存在すること。
- `pub` の露出範囲が最小限か、`pub(crate)` や非公開で済むものが `pub` になっていないか確認する。
- 10本のカスタムlint（mod-file, module-examples, module-wiring, type-per-file, tests-location, use-placement, redundant-fqcn, rustdoc, cfg-std-forbid, ambiguous-suffix）に準拠しているか確認する。
- 公開API（`pub struct`, `pub trait`, `pub enum`, `pub fn`）に英語の rustdoc があるか確認する。
- `unsafe` ブロックがある場合は必要性と安全性コメントを確認する。
- `std` / `no_std` の feature flag 設定と `cfg-std-forbid` lint の整合性を確認する。
- 不要な外部クレート依存、既存依存で代替できる新規依存、core クレートの no_std 非互換プロダクト依存がないか確認する。

## 設計判断の参照

{report:coder-decisions.md} を確認し、記録された設計判断を把握してください。

- 記録された意図的な判断は FP として指摘しない。
- ただし設計判断自体の妥当性も評価し、問題がある場合は指摘する。

## 前回指摘の追跡

- まず「Previous Response」から前回の open findings を抽出する。
- 各 finding に `finding_id` を付け、今回の状態を `new / persists / resolved / reopened` で判定する。
- `persists` または `reopened` と判定する場合は、未解決または再発の根拠（ファイル/行）を必ず示す。
