---
name: pekko-gap-analysis
description: >-
  fraktor-rsの指定モジュール（modules/{name}）とApache Pekkoの参照実装（references/pekko/{name}）を比較し、
  不足機能を洗い出すギャップ分析スキル。公開API・trait・オペレーター・パターンを両側から抽出し、
  カテゴリ別に分類して難易度を付与する。制約カード（spec-constraint-card）作成の入力として活用可能。
  トリガー：「Pekkoと比較して不足機能を洗い出して」「gap analysis」「ギャップ分析」
  「references/pekkoとの差分」「不足オペレーターを調べて」「Pekko対応状況」
  「modules/{name}の不足機能」といったPekko参照実装との比較リクエストで起動。
---

# Pekko ギャップ分析

`modules/{name}` と `references/pekko/{name}` を比較し、不足機能を体系的に洗い出す。

## 引数

モジュール名をケバブケースまたはスネークケースで受け取る。

```
/pekko-gap-analysis streams
/pekko-gap-analysis actor
/pekko-gap-analysis cluster
```

## fraktor-rs のアーキテクチャ層構造

層構造の詳細は `.agents/rules/rust/module-structure.md` を参照すること。
分析においては、そこで定義された core / std / embedded の分離と、
core 内部の untyped kernel / typed ラッパーの区別を正確に反映すること。

### Pekko との層マッピング

| Pekko | fraktor-rs |
|-------|------------|
| `pekko-actor/classic` (untyped) | `modules/actor/src/core/actor/` (untyped kernel) |
| `pekko-actor-typed` | `modules/actor/src/core/typed/` (typed ラッパー) |
| `pekko-stream` | `modules/streams/src/core/` |
| ランタイム固有実装 | `modules/{name}/src/std/` |

**注意**: すべてのモジュールが `typed/` サブ層を持つわけではない。
`list_dir` で実際の構造を確認してからマッピングを決定すること。

## ワークフロー

### 1. 対象ディレクトリの特定と層構造の把握

引数 `{name}` から以下のパスを導出する：

- **fraktor-rs側**: `modules/{name}/src/`
- **Pekko側**: `references/pekko/{name}/src/`

両方のディレクトリが存在することを確認する。存在しない場合はユーザーに報告して終了。

Pekko側のディレクトリ名が異なる場合（例: `stream` vs `streams`）は、
`references/pekko/` 配下を `list_dir` で確認して対応するディレクトリを特定する。

**層構造の把握（必須）**:
```
# fraktor-rs側の層構造を確認
list_dir: modules/{name}/src/core/
list_dir: modules/{name}/src/std/

# typed サブ層の有無を確認
list_dir: modules/{name}/src/core/typed/  (存在する場合)
list_dir: modules/{name}/src/std/typed/   (存在する場合)
```

### 2. Pekko側のAPI抽出

Pekko参照実装から公開APIを抽出する。

```
# Scala の公開型・トレイト・オブジェクトを列挙（修飾子・アノテーション付きを含む）
search_for_pattern: "^\s*(?:final |sealed |abstract |private |protected )*(?:case )?(?:class|trait|object|enum)\s+"
  relative_path: references/pekko/{name}/src/
  restrict_search_to_code_files: true

# 主要な公開メソッドを列挙（修飾子・記号メソッド含む）
search_for_pattern: "^\s+(?:final |override |protected |private )*def\s+\S+"
  relative_path: references/pekko/{name}/src/
  restrict_search_to_code_files: true
```

抽出後、`private` / `private[...]` 修飾付きのものはフィルタで除外する。

抽出結果を以下のカテゴリに分類する：

| カテゴリ | 例（streams の場合） |
|----------|----------------------|
| 型・トレイト | Source, Flow, Sink, Graph, Shape |
| オペレーター | map, filter, flatMap, merge, zip |
| マテリアライゼーション | Keep, viaMat, toMat |
| グラフDSL | GraphDSL.Builder, fan-in/fan-out |
| ライフサイクル | KillSwitch, watchTermination |
| エラー処理 | recover, recoverWith, supervision |
| その他 | ユーティリティ、設定、テストキット |

### 3. fraktor-rs側のAPI抽出（層別）

fraktor-rs実装から **層ごとに** 公開APIを抽出する。

```
# core層（方針）の公開型を列挙
search_for_pattern: "^\s*pub(?:\([^)]*\))?\s+(?:struct|trait|enum|type)\s+"
  relative_path: modules/{name}/src/core/
  restrict_search_to_code_files: true

# std層（詳細）の公開型を列挙
search_for_pattern: "^\s*pub(?:\([^)]*\))?\s+(?:struct|trait|enum|type)\s+"
  relative_path: modules/{name}/src/std/
  restrict_search_to_code_files: true

# 公開メソッドも同様に層別で列挙
search_for_pattern: "^\s*pub(?:\([^)]*\))?\s+(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?fn\s+\w+"
  relative_path: modules/{name}/src/core/
  restrict_search_to_code_files: true

search_for_pattern: "^\s*pub(?:\([^)]*\))?\s+(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?fn\s+\w+"
  relative_path: modules/{name}/src/std/
  restrict_search_to_code_files: true
```

typed サブ層が存在する場合は、さらに細分化する：

```
# core/typed 層（typed ラッパー）
search_for_pattern: "^\s*pub(?:\([^)]*\))?\s+(?:struct|trait|enum|type)\s+"
  relative_path: modules/{name}/src/core/typed/
  restrict_search_to_code_files: true

# core の typed 以外（untyped kernel）
# → core/ 全体から typed/ を除いた結果を使う
```

同じカテゴリ体系で分類し、各APIに **所属層** を付記する。

### 4. ギャップの特定（層を考慮）

Pekkoに存在してfraktor-rsに存在しない機能を特定する。

判定基準（名前だけでなくシグネチャ・セマンティクスを総合的に確認する）：
- **実装済み**: 同名または同等のシグネチャを持つメソッド/型が存在し、同じ契約を満たす
- **別名で実装済み**: 名前は異なるが同じ機能を提供 → 対応シンボルを明記して根拠を示す
- **部分実装**: シグネチャは存在するが本体が `todo!()` / `unimplemented!()` → スタブ
- **未実装**: 対応する機能が存在しない → ギャップ

**層の判定（必須）**: 各ギャップについて、実装すべき層を判定する：

| 判定基準 | 実装先 |
|----------|--------|
| コアロジック・trait定義・no_stdで実現可能 | core |
| tokio/std依存・ネットワーク・ファイルIO | std |
| 型パラメータ化されたラッパーAPI | core/typed |
| untyped kernel の拡張 | core/{domain} |

### 5. 難易度の分類

各ギャップに対して実装難易度を付与する：

| 難易度 | 基準 |
|--------|------|
| trivial | 既存APIの組み合わせ・委譲のみで実装可能。新規公開型の追加なし |
| easy | 新規公開型1-2個の追加で実装可能。既存traitの拡張不要 |
| medium | 新規公開型3個以上、または既存traitの拡張が必要 |
| hard | アーキテクチャ変更・基盤レイヤーの修正を伴う |
| n/a | Rust/no_stdの制約上実装不要、JVM固有、またはdeprecated |

### 6. 結果の出力

以下のフォーマットで結果を出力する：

```markdown
# {name} モジュール ギャップ分析

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | N（型単位で計数、オーバーロードは1つとして集約） |
| fraktor-rs 公開型数 | M（core: X, std: Y） |
| カバレッジ（型単位） | M/N (XX%) |
| ギャップ数 | G（core: Gc, std: Gs） |

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | N1 | M1 | XX% |
| core / typed ラッパー | N2 | M2 | XX% |
| std / アダプタ | N3 | M3 | XX% |

## カテゴリ別ギャップ

各カテゴリのヘッダーには **実装済み数 / Pekko総数 (カバレッジ%)** を明記する。
これによりサマリーのカバレッジ数値の根拠をカテゴリ単位で検証できる。

ギャップ（未対応・部分実装・n/a）のみテーブルに列挙する。
実装済みはカテゴリの件数カウントに含めるが、テーブル行には追加しない。

### カテゴリ名　✅ 実装済み X/Y (ZZ%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `methodName` | `Flow.scala:L123` | 未対応 | core/typed | easy | 説明 |
| `ClassName` | `Graph.scala:L45` | 部分実装 | core/kernel | trivial | `foo.rs:L12` にスタブあり |
| `RuntimeX` | `Runtime.scala:L78` | 未対応 | std | medium | tokio依存 |

### ...（カテゴリごとに繰り返し）

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）
- 各項目に実装先層（core/kernel, core/typed, std）を明記
- ...

### Phase 2: easy（単純な新規実装）
- ...

### Phase 3: medium（中程度の実装工数）
- ...

### Phase 4: hard（アーキテクチャ変更を伴う）
- core層の変更が必要な場合は特に明記（std層への波及を評価）
- ...

### 対象外（n/a）
- ...

## まとめ

必ず最後にまとめセクションを出力すること。以下の内容を含める：

- 全体カバレッジの一言評価（「主要機能はカバー済み」「基盤部分が手薄」等）
- 即座に価値を提供できる未実装機能（Phase 1〜2 の代表例）
- 実用上の主要ギャップ（Phase 3〜4 の代表例）
- YAGNI観点での省略推奨（意図的な非実装と判断できるもの）
```

## 注意事項

- 出力したファイルがあるか必ず`ls`コマンドで確認すること
- Pekkoの全機能を移植することが目的ではない（YAGNI原則）
- 結果は「何が足りないか」の可視化であり、すべてを実装すべきという提案ではない
- `n/a` 判定は保守的に行う（JVM固有、Akka互換層、deprecated機能のみ）
- Rust/no_std 固有の制約（`cfg_std_forbid` lint等）を考慮する

## 関連スキル

- **reviewing-fraktor-types**: 既存実装の型設計レビュー（過剰設計の検出）
- **creating-fraktor-modules**: 新規モジュール・型の雛形生成
