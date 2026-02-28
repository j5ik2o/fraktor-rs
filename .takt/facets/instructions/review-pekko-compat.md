**Pekko互換性**と**fraktor-rs規約準拠**のレビューに集中してください。

## やらないこと (Do Not)

- `cargo check` / `cargo build` / `cargo test` など、ビルドを伴うコマンドを実行しないこと。このムーブメントはビルド権限がなく `Operation not permitted` で失敗する。ビルド検証は `fix` / `implement` ムーブメントの責務。
AI特有の問題はレビューしないでください（ai_reviewムーブメントで実施済み）。

**レビュー観点:**
- Pekko参照実装とのAPI対応関係の正確性
- Scala→Rust変換パターンの適切性
- 型パラメータ（TB: RuntimeToolbox）の正しい配置
- no_std/std分離の妥当性
- CQS原則（&self vs &mut self）の遵守
- 1ファイル1公開型ルールの遵守
- 命名規約（snake_case メソッド、*Generic サフィックス等）
- テストの存在と妥当性
- YAGNI（タスク範囲外の不要な機能追加がないこと）

**Pekko参照の確認（必須）:**
- `references/pekko/` の該当ソースを実際に開いて、実装との対応を検証する
   - clusterモジュールはpekkoだけではなく`references/protoactor-go`の該当ソースを開くこと
- 推測で「互換」と判定しない

**前回指摘の追跡（必須）:**
- まず「Previous Response」から前回の open findings を抽出する
- 各 finding に `finding_id` を付け、今回の状態を `new / persists / resolved` で判定する
- `persists` と判定する場合は、未解決である根拠（ファイル/行）を必ず示す

## 判定手順

1. まず前回open findingsを抽出し、`new / persists / resolved` を仮判定する
2. Pekko参照実装を読み、実装されたAPIとの対応関係を確認する
3. 変換パターンの正確性を検証する（ナレッジの変換ルール表を参照）
4. fraktor-rs固有の制約（Dylint lint、CQS、命名規約）への準拠を確認する
5. ブロッキング問題（`new` または `persists`）が1件でもあればREJECTと判定する
