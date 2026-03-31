---
name: pekko-gap-analysis
description: >-
  fraktor-rsの指定モジュール（modules/{name}）とApache Pekkoの参照実装（references/pekko/{name}）を比較し、
  不足機能を洗い出すギャップ分析スキル。公開API・trait・オペレーター・パターンを両側から抽出し、
  カテゴリ別に分類して難易度を付与する。APIレベルのギャップが少ない場合は、内部モジュール構造
  （責務分割・層配置・依存方向）の差分も分析する。
  トリガー：「Pekkoと比較して不足機能を洗い出して」「gap analysis」「ギャップ分析」
  「references/pekkoとの差分」「不足オペレーターを調べて」「Pekko対応状況」
  「modules/{name}の不足機能」といったPekko参照実装との比較リクエストで起動。
---

# Pekko ギャップ分析

`modules/{name}` と `references/pekko/{name}` を比較し、不足機能を体系的に完全に洗い出す。
※YAGNIはここでは適用しないこと。完了のための計画を出す必要があります。中途半端な計画はNGです。

分析は以下の二段階で行う：

1. まず公開APIレベルのギャップを分析する
2. APIギャップが十分に少ない場合のみ、内部モジュール構造のギャップ分析に進む

APIギャップが大きい段階では、内部構造の差分よりも公開契約の不足解消が優先である。
逆にAPIギャップが小さい段階では、次の改善余地は「内部責務の切り方」「層配置」「依存方向」に移る。

## 引数

モジュール名をケバブケースまたはスネークケースで受け取る。

```
/pekko-gap-analysis stream
/pekko-gap-analysis actor
/pekko-gap-analysis cluster
```

## fraktor-rs のアーキテクチャ層構造

層構造の詳細は `.agents/rules/rust/module-structure.md` を参照すること。
分析においては、そこで定義された core / std / embedded の分離と、
core 内部の untyped kernel / typed ラッパーの区別を正確に反映すること。
std/embedded はcoreのポートを実装するアダプタモジュールです。

### Pekko との層マッピング

| Pekko                   | fraktor-rs |
|-------------------------|------------|
| `pekko/actor` (untyped) | `modules/actor/src/core/kernel/` (untyped kernel) |
| `pekko/actor-typed`     | `modules/actor/src/core/typed/` (typed ラッパー) |
| `pekko-stream`          | `modules/stream/src/core/` |
| ランタイム固有アダプタ実装  | `modules/{name}/src/std/` |

**注意**: すべてのモジュールが `typed/` サブ層を持つわけではない。
`list_dir` で実際の構造を確認してからマッピングを決定すること。

## ワークフロー


※必要に応じて、sub-agents, multi-agentsを使って効率的に調査してもよい。

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

### 5. APIギャップが少ないかの判定

以下のいずれかを満たす場合、**APIレベルの主要ギャップは概ね埋まっている** とみなし、
内部モジュール構造ギャップ分析に進む：

- 型単位カバレッジが 80% 以上
- `hard` / `medium` の未実装ギャップが 5件以下
- 主要カテゴリ（型・主要オペレーター・ライフサイクル）ごとに、致命的な欠落が 0 件、かつカテゴリごとの未実装ギャップが 1 件以下

上記を満たさない場合は、内部モジュール構造分析は省略してよい。
その場合は「APIギャップが支配的であり、構造比較は後続フェーズ」と明記すること。

### 6. 内部モジュール構造ギャップ分析

APIギャップが少ない場合のみ実施する。
目的は、**公開APIでは見えないが、今後の実装速度・保守性・責務境界に影響する差分** を洗い出すこと。

#### 6.1 対象

- fraktor-rs 側: `modules/{name}/src/core/`, `modules/{name}/src/std/`, 必要に応じて `core/typed/`
- Pekko 側: `references/pekko/{name}/src/main/scala/...` 配下の内部パッケージ・補助型・実装詳細

#### 6.2 抽出観点

以下の観点で、Pekko側の内部モジュール構造を抽出する：

| 観点 | 確認内容 |
|------|----------|
| 責務分割 | interpreter / stage / logic / materializer / dispatcher などの分離有無 |
| 層配置 | 公開API層と内部実装層がどう分かれているか |
| 依存方向 | 上位DSL → 下位実行基盤への片方向依存になっているか |
| 共有内部部品 | 複数APIで再利用される内部抽象が存在するか |
| 実装の集約点 | 機能追加時に中心となる内部モジュールがどこか |
| テスト支援構造 | testkit / utilities / adapters が本体からどう分離されているか |

#### 6.3 調査手順

```bash
# Pekko 側の内部ディレクトリと Scala ファイルを把握
find references/pekko/{name}/src -maxdepth 4 -type d | sort
find references/pekko/{name}/src -name "*.scala" | sort

# fraktor-rs 側の内部ディレクトリを把握
find modules/{name}/src -maxdepth 4 -type d | sort
find modules/{name}/src -name "*.rs" | sort
```

必要に応じて `search_for_pattern` で以下を確認する：

- `package `
- `trait `
- `final class `
- `object `
- `mod `
- `pub(crate)`

#### 6.4 比較方法

以下の単位で比較する：

1. **モジュール境界**: Pekkoの内部責務に対して、fraktor-rsに対応するサブモジュールがあるか
2. **責務の置き場所**: ある責務が `core` にあるべきか `std` にあるべきかが妥当か
3. **typed/untyped の分離**: fraktor-rsの `core/typed` が薄いラッパーに留まっているか
4. **実装集約の不足/過剰**: 1責務が分散しすぎていないか、逆に1モジュールへ詰め込みすぎていないか
5. **将来の拡張経路**: 新規APIを追加する際の受け皿になる内部モジュールが存在するか

#### 6.5 構造ギャップの判定

以下のいずれかに該当する場合は、構造ギャップとして記録する：

- Pekkoでは独立責務として分離されているが、fraktor-rsでは未分離で責務が混在している
- fraktor-rsに対応モジュールはあるが、`core` / `std` / `core/typed` の置き場所が不自然
- typed 層が薄いラッパーを超えて重いロジックを抱えている
- 同じ責務が複数サブモジュールへ分散し、変更点が集約されていない
- 実装追加時の拡張ポイントが見えず、都度散発的に追加される構造になっている

逆に、以下は構造ギャップにしない：

- Rust/no_std 制約により Pekko と同じパッケージ分割が成立しない
- 公開API差分を吸収するための一時的な内部差分で、責務境界が明確
- Less is more / YAGNI の観点で、未使用の細分化を意図的に持たない

#### 6.6 構造ギャップの出力粒度

構造ギャップごとに次を明記する：

| 項目 | 内容 |
|------|------|
| ギャップ名 | 例: `materialization責務の集約点不足` |
| Pekko側の根拠 | 対応する package / class / trait / object |
| fraktor-rs側の現状 | 対応モジュール、または未配置 |
| 問題の種類 | 未分離 / 誤配置 / 責務分散 / 過剰分割 |
| 推奨アクション | 新規サブモジュール追加 / 既存モジュールへ集約 / core↔stdの再配置 |
| 緊急度 | low / medium / high |

### 7. 難易度の分類

各ギャップに対して実装難易度を付与する：

| 難易度 | 基準 |
|--------|------|
| trivial | 既存APIの組み合わせ・委譲のみで実装可能。新規公開型の追加なし |
| easy | 新規公開型1-2個の追加で実装可能。既存traitの拡張不要 |
| medium | 新規公開型3個以上、または既存traitの拡張が必要 |
| hard | アーキテクチャ変更・基盤レイヤーの修正を伴う |
| n/a | Rust/no_stdの制約上実装不要、JVM固有、またはdeprecated |

構造ギャップにも同じ難易度を適用してよいが、難易度の意味は以下で補正する：

- `trivial`: 既存モジュールへの責務移動や再配置だけで済む
- `easy`: 新規サブモジュール1-2個の追加で済む
- `medium`: 複数モジュールにまたがる責務再編が必要
- `hard`: core/std 境界や typed/untyped 方針の見直しが必要

### 8. 結果の出力

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

## 内部モジュール構造ギャップ

APIギャップが少ないと判定した場合のみ、このセクションを追加する。
該当しない場合は「今回はAPIギャップが支配的なため省略」と明記する。

| 構造ギャップ | Pekko側の根拠 | fraktor-rs側の現状 | 推奨アクション | 難易度 | 緊急度 | 備考 |
|-------------|---------------|--------------------|----------------|--------|--------|------|
| `責務名` | `impl/package/path` | `modules/...` | `coreへ集約` | medium | high | 説明 |

## 実装優先度

この節のルール:
- ここで出す優先度は「今の要求で実装すべきか」ではなく、「Pekko parity ギャップをどの順で埋めるか」を示す
- この節では **YAGNI を適用しない**。未要求でも parity ギャップなら優先順位付けの対象に含める
- 新しいフェーズ名、追加軸、思いつきの派生提案を増やしてはならない
- 優先度へ載せる項目は、必ず直前の「カテゴリ別ギャップ」に列挙済みの項目だけに限定する
- したがって、この節は「新しい提案」ではなく「既存ギャップの再配置」でなければならない

分類ルール:
- Phase 1: trivial / easy。既存設計の範囲で API surface や placeholder を埋められるもの
- Phase 2: medium。追加ロジックは要るが、既存の core / std 境界の中で閉じるもの
- Phase 3: hard。低レベル stage authoring、新規 transport、remote 連携など、新しい基盤やアーキテクチャ変更を要するもの
- 対象外（n/a）: JVM 固有、Java 相互運用専用、deprecated など parity 対象外のもの

出力制約:
- 各 Phase には、カテゴリ別ギャップから再掲した項目のみを書く
- 各項目には実装先層（core, core/typed, std）を必ず付記する
- 「別案」「将来的には」「追加で考えられる」など、ギャップ表にない派生提案を書いてはならない
- ギャップ数が少なくても Phase を増やさず、既存の Phase 1-3 のどれかへ入れる

## まとめ

必ず最後にまとめセクションを出力すること。以下の内容を含める：

- 全体カバレッジの一言評価（「主要機能はカバー済み」「基盤部分が手薄」等）
- parity を低コストで前進できる未実装機能（Phase 1〜2 の代表例）
- parity 上の主要ギャップ（Phase 3 の代表例）
- APIギャップが少ない場合は、次のボトルネックが内部構造にあるかどうかの一言評価
```

## 注意事項

- ほとんどのロジックはcoreに集中している前提
- std/embedded はcoreのポートを実装するアダプタモジュール
- 完了のための計画になっているか確認し、漏れがある場合は是正すること
- 出力したファイルを `ls -al ${PROJECT_ROOT}/docs/gap-analysis/${name}-gap-analysis.md` コマンドで更新されているか確認すること
- `n/a` 判定は保守的に行う（JVM固有、Akka互換層、deprecated機能のみ）
- Rust/no_std 固有の制約（`cfg_std_forbid` lint等）を考慮する

## 関連スキル

- **reviewing-fraktor-types**: 既存実装の型設計レビュー（過剰設計の検出）
- **creating-fraktor-modules**: 新規モジュール・型の雛形生成
