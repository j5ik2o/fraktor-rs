品質保証の観点から変更をレビューしてください。

## レビュー観点

- テストカバレッジと品質
- テスト戦略（単体/統合/E2E）
- エラーハンドリング
- ログとモニタリング
- 保守性

### モジュール構成

- コアロジックである`core`モジュール(`modules/*/src/core`)は`no_std`で記述すること
- プラットフォーム依存部分は`std`モジュール(`modules/*/src/std`)や`embedded`(`modules/*/src/embedded`)に記述すること
  - `std`モジュール: std依存APIやtokioなど
  - `embedded`モジュール: マイコンやIoT向けのAPIなど

たとえば、

- 本来coreモジュールに実装できるロジックがstdモジュールなどに配置されていないか？
- coreモジュールに実装できるロジックがstdモジュールなどに配置されていないか？

### examplesの網羅性

- `examples`の網羅性は十分か、利用者の視点で考える
- `modules/*/src/**/*.rs`に対応する`examples/*/src/**/*.rs`が存在すること
- `example`のコードを書いてみて、複雑で長いコードを書かざるを得ない場合は、プロダクトコードのインターフェイスや設計が不十分である可能性が高いため、コードを簡潔にできるように設計を見直さなければならない

### 公開APIの最小化

- `pub` の露出範囲が最小限か
- `pub(crate)` や非公開で済むものが `pub` になっていないか
- 不要な型・メソッド・フィールドが公開されていないか

### Dylint lint準拠

- 8つのカスタムlint がパスしているか
  - mod-file, module-wiring, type-per-file, tests-location, use-placement, rustdoc, cfg-std-forbid, ambiguous-suffix
- `#[allow]` による lint 回避が人間の許可なく行われていないか

### rustdocの存在と言語

- 公開API（`pub struct`, `pub trait`, `pub enum`, `pub fn`）に rustdoc（`///`）が記述されているか
- rustdoc は英語で記述されているか（それ以外のコメント・Markdownは日本語）

### unsafe使用の妥当性

- `unsafe` ブロックがある場合、その必要性が明確か
- 安全性の根拠（なぜ未定義動作が起きないか）がコメントで説明されているか
- `unsafe` を使わずに実現できる代替手段がないか

### feature flagの整合性

- `std` / `no_std` の feature flag 設定が正しいか
- `core` モジュール（`modules/*/src/core`）が意図せず `std` に依存していないか
- `cfg-std-forbid` lint と合わせて、feature gate の漏れがないか

### 依存クレートの妥当性

- 不要な外部クレート依存が追加されていないか（YAGNI観点）
- 既存の依存で代替できる機能に対して新規クレートが追加されていないか
- `no_std` 互換でないクレートが `core` モジュールの依存に含まれていないか

---

## 設計判断の参照

{report:coder-decisions.md} を確認し、記録された設計判断を把握してください。
- 記録された意図的な判断は FP として指摘しない
- ただし設計判断自体の妥当性も評価し、問題がある場合は指摘する

## 前回指摘の追跡（必須）

- まず「Previous Response」から前回の open findings を抽出する
- 各 finding に `finding_id` を付け、今回の状態を `new / persists / resolved` で判定する
- `persists` と判定する場合は、未解決である根拠（ファイル/行）を必ず示す

## 判定手順

1. まず前回open findingsを抽出し、`new / persists / resolved` を仮判定する
2. 変更差分を確認し、品質保証の観点に基づいて問題を検出する
  - ナレッジの判定基準テーブル（REJECT条件）と変更内容を照合する
3. 検出した問題ごとに、Policyのスコープ判定表と判定ルールに基づいてブロッキング/非ブロッキングを分類する
4. ブロッキング問題（`new` または `persists`）が1件でもあればREJECTと判定する
