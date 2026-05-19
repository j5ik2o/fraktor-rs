# AIレビュー指示（パッケージ構造リファクタリング）

## やらないこと (Do Not)

- `cargo check` / `cargo build` / `cargo test` など、ビルドを伴うコマンドを実行しないこと。このムーブメントはビルド権限がなく `Operation not permitted` で失敗する。ビルド検証は `implement` / `ai_fix` / `fix` の責務。

## やること (Do)

1. AI生成コード特有の問題を対象ファイルで確認する。主に、幻覚API、ファントムインポート、パターン補完エラー、過度な抽象化、未使用デッドコード、フォールバック濫用、指示外の後方互換追加をチェックする
2. パッケージ構造リファクタリングでは、次を **ブロッキング問題** として重点確認する
   - `#[path = "..."]`
   - `include!`
   - 旧モジュールから新モジュールへの互換 `pub use` / `pub type` / wrapper mod
   - 同一責務の旧パス / 新パスの二重維持
   - import を更新せず、親モジュールや crate root の再エクスポートだけで見かけ上通した跡
   - `structure-design.md` で許可されていない公開境界 re-export
   - 実装未着手、またはレポートだけで完了扱いにしている状態
3. `structure-design.md` と `plan.md` を読み、各責務の **正準パス** と **許容された re-export** を抽出し、実装が一致しているか確認する
4. `Previous Response` から前回の open findings を抽出して、各 finding に `finding_id` を付与する
5. 各 finding を `new / persists / resolved` で判定する。`persists` の場合は、未解決の根拠（ファイル/行）を示す
6. ブロッキング問題（`new` または `persists`）が1件でもある場合は REJECT、0件なら APPROVE を判定する

## 必須出力 (Required Output)

1. 変更した点とその根拠を、finding ごとに明記する
2. 最終判定を `REJECT` または `APPROVE` で示す
3. `REJECT` の場合は、必ずブロッキング issue の file/line 付きで修正方針を示す
