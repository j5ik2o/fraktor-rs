# Pekko ギャップ分析レビュー指示

## やらないこと (Do Not)
- ビルドコマンド（`cargo check` / `cargo build` / `cargo test`）を実行しないこと

## やること (Do)
1. `03-pekko-gap-analysis.md`（統合レポート）を読み込み、以下の観点で検証する
2. 必要に応じて `00-api-inventory.md`、`01-interface-comparison.md`、`02-design-comparison.md` を参照し裏付けを取る
3. **ソースコードでの裏付け**: 判定の正確性を検証するため、実際のソースファイルを Read で確認する（サンプリング検証でよい）
4. 前回のレビューレポート `04-pekko-gap-review.md` が存在する場合はそこから前回の open findings を抽出し、各 finding に `finding_id` を付与する（初回レビュー時は全て `new`）
5. 各 finding を `new / persists / resolved` で判定する
6. ブロッキング問題が1件でもあれば REJECT、0件なら APPROVE

## レビュー観点

| 観点 | チェック内容 |
|------|-------------|
| Layer 1 網羅性 | Pekko側の主要な公開型・トレイトが漏れなく抽出されているか |
| Layer 1 正確性 | fraktor-rs側の公開APIが正しく抽出されているか |
| Layer 2 対応正確性 | 「同名実装」「別名実装」「部分実装」の判定が正しいか（対応シンボルの存在をソースで確認） |
| Layer 2 シグネチャ | 引数型・戻り値型・型パラメータの比較が正確か |
| Layer 3 設計判断 | トレイト階層・抽象化パターンの評価が根拠を伴っているか |
| Layer 3 難易度 | 各ギャップの難易度が根拠と整合しているか |
| n/a判定 | JVM固有・deprecated以外を安易にn/aにしていないか |
| カバレッジ | サマリーの数値が本文の内容と一致しているか |
| YAGNI | 不要な機能を「必須」として挙げていないか |
| Layer間整合性 | Layer 1〜3 の間で矛盾がないか |

## 必須出力
1. 各 finding とその根拠を明記する
2. 最終判定を `REJECT` または `APPROVE` で示す
3. `REJECT` の場合は修正方針を具体的に示す（どの Layer のどの箇所を修正すべきか）
