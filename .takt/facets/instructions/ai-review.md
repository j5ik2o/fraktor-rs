AI生成コード特有の問題をレビューせよ。

**レビュー観点:** 幻覚API・ファントムインポート・パターン補完エラー・過度な抽象化・未使用デッドコード・フォールバック濫用・指示外の後方互換追加

**前回指摘の追跡（必須）:** Previous Response から前回の open findings を抽出し、各 finding に `finding_id` を付け `new / persists / resolved` で判定する。`persists` には未解決の根拠（ファイル/行）を示す。

**判定:** 変更対象ファイルでアンチパターンを確認し、ブロッキング問題（`new` / `persists`）が1件でもあれば REJECT、0件なら APPROVE。
