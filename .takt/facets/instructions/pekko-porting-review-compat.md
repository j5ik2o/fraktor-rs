**Pekko互換性**と**fraktor-rs規約準拠**のレビューに集中してください。

## 最重要方針

レビュー対象は「Pekko に似ているか」ではなく、
**Pekko の契約意図が Rust / fraktor-rs の設計原則を壊さずに再表現されているか** である。

## やらないこと (Do Not)

- `cargo check` / `cargo build` / `cargo test` など、ビルドを伴うコマンドを実行しないこと。このステップはビルド権限がなく `Operation not permitted` で失敗する。ビルド検証は `fix` / `implement` ステップの責務。
AI特有の問題はレビューしないでください（ai_reviewステップで実施済み）。

**レビュー観点:**
- Rust として不自然な API を導入していないか
- Pekko参照実装とのAPI対応関係の正確性
- Scala→Rust変換パターンの適切性
- 型パラメータ（メッセージ型 M 等）の正しい配置
- no_std/std分離の妥当性
- CQS原則（&self vs &mut self）の遵守
- 1ファイル1公開型ルールの遵守
- 命名規約（snake_case メソッド、*Shared サフィックス等）
- テストの存在と妥当性
- YAGNIを重視（タスク範囲外の不要な機能追加がないこと）
- YAGNIを悪用して中途半端な作業をしないこと

**Pekko参照の確認（必須）:**
- `references/pekko/` の該当ソースを実際に開いて、実装との対応を検証する
   - clusterモジュールはpekkoだけではなく`references/protoactor-go`の該当ソースを開くこと
- 推測で「互換」と判定しない
- wrapper / alias だけで互換面を偽装していないか確認する
- `ignore()` / `empty()` / `self` を返すだけの fallback public API がないか確認する
- no-op / placeholder のまま Pekko互換名を public にしていないか確認する
- `public API` と `internal implementation` の境界が悪化していないか確認する

**前回指摘の追跡（必須）:**
- まず「Previous Response」から前回の open findings を抽出する
- 各 finding に `finding_id` を付け、今回の状態を `new / persists / resolved` で判定する
- `persists` と判定する場合は、未解決である根拠（ファイル/行）を必ず示す

## 判定手順

1. まず前回open findingsを抽出し、`new / persists / resolved` を仮判定する
2. Pekko参照実装を読み、実装されたAPIとの対応関係を確認する
3. 変換パターンの正確性を検証する（ナレッジの変換ルール表を参照）
4. fake parity（見た目だけ互換）に該当しないか確認する
5. fraktor-rs固有の制約（Dylint lint、CQS、命名規約）への準拠を確認する
6. ブロッキング問題（`new` または `persists`）が1件でもあればREJECTと判定する
