# Layer 1: API インベントリ抽出

`{task}` で指定されたモジュールについて、**事実の列挙のみ** を行う。判定・評価は後続ステップの責務。

## やらないこと (Do Not)
- ビルドコマンド（`cargo check` / `cargo build` / `cargo test`）を実行しないこと
- このムーブメントはソースコードの静的解析のみを行う

## 対象パスの導出

- **fraktor-rs側**: `modules/{name}/src/`
- **Pekko側**: `references/pekko/{name}/src/`
  - ディレクトリ名が異なる場合（例: `stream` vs `streams`）は `references/pekko/` 配下を確認して特定
  - Pekko側のモジュールが複数ディレクトリに分割されている場合（例: `actor/` + `actor-typed/`）は、
    主要なディレクトリ（`-typed` 付き）を優先し、必要に応じて他のディレクトリも参照する

両方の存在を確認する。存在しない場合は報告して終了。

## Pekko側の抽出

以下のパターンで検索し、`private` / `private[...]` 修飾付きを除外する：

```
# 公開型・トレイト・オブジェクト
search: "^\s*(?:final |sealed |abstract |protected )*(?:case )?(?:class|trait|object|enum)\s+"

# 公開メソッド
search: "^\s+(?:final |override |protected )*def\s+\S+"
```

## fraktor-rs側の抽出

```
# 公開型
search: "^\s*pub(?:\([^)]*\))?\s+(?:struct|trait|enum|type)\s+"

# 公開メソッド
search: "^\s*pub(?:\([^)]*\))?\s+(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?fn\s+\w+"
```

## 出力ルール

- **判定しない**: 「実装済み」「未実装」等の判断は行わない
- **ファイルパスと行番号を必ず記録する**: 後続ステップの検証に必要
- **シグネチャを省略しない**: 型パラメータ、引数型、戻り値型を含めて記録する
- カテゴリ分類は対象モジュールの内容に合わせて行う：
  - actor モジュール例: 型・トレイト / Behavior / メッセージ処理 / ライフサイクル / 監視戦略 / エラー処理 / その他
  - streams モジュール例: 型・トレイト / オペレーター / マテリアライゼーション / グラフDSL / ライフサイクル / エラー処理 / その他
  - cluster モジュール例: 型・トレイト / メンバーシップ / ルーティング / PubSub / エラー処理 / その他
  - 上記にない場合はモジュール構造から適切なカテゴリを導出する

## 判定

- 両側のAPIインベントリを出力完了: `抽出完了`
- 対象ディレクトリが存在しない: `対象不明`
