# Phase 5: ルール運用見直し — 分析レポート

**作成日**: 2026-02-06
**分析対象**: プロジェクト全体のルール・スキル・lint 体系
**前提**: Phase 1-4（公開API監査、message_adapter統合、tick_driver保全、mailbox整理）完了済み

---

## エグゼクティブサマリー

Phase 1-4 でコードの過剰設計は改善されたが、**ルール自体の運用**に以下の課題が残っている：
1. 同じルールが5箇所以上に散在・重複し、矛盾リスクとコンテキスト浪費が生じている
2. 「1 file = 1 public type」の例外基準が未定義のまま
3. lint でカバーされていない重要ルール（CQS、内部可変性、曖昧サフィックス）がある
4. プロジェクト固有のスキル（`.claude/skills/`）が完全に不在

---

## 1. 現状の課題マップ

### 課題A: ルールの散在と重複（最重要）

同じルールが **5箇所以上** に分散している：

| ルール | CLAUDE.md | steering | okite-ai | Serena memory | Dylint |
|--------|-----------|----------|----------|--------------|--------|
| 1ファイル1公開型 | ✅ | structure.md | single-type-per-file.md | ✅ | type-per-file-lint |
| mod.rs禁止 | ✅ | structure.md | - | ✅ | mod-file-lint |
| 内部可変性禁止 | ✅ | tech.md | - | ✅ | - |
| YAGNI/Less is more | ✅ | tech.md | less-is-more.md | - | - |
| 曖昧サフィックス禁止 | ✅(tech.md) | tech.md | avoiding-ambiguous-suffixes.md | ✅ | - |
| FQCN import | ✅ | structure.md | - | ✅ | module-wiring-lint |
| テスト配置 | ✅ | structure.md | - | ✅ | tests-location-lint |

**問題**: AIは毎セッション開始時にこれらすべてを読む。重複による矛盾リスクと、コンテキストウィンドウの無駄遣いが生じている。

### 課題B: 「1 file = 1 public type」の例外基準が未定義

Phase 1-4 の実績から分かったこと：
- 30行以下のファイルが全体の **35%**（138ファイル）
- `type-per-file-lint` は機械的に強制するが、「同居すべきケース」の判断基準がない
- Phase 2-4 で公開型を削減したが、ルール自体は変更されていない

**核心の問い**: 小型の設定enum/newtypeは、関連する公開型と同居させるべきか？

### 課題C: lint でカバーされていない重要ルール

| ルール | 現状の執行方法 | 問題 |
|--------|--------------|------|
| CQS原則 | CLAUDE.md に記載のみ | AIが見落とすリスク |
| &mut self vs &self判断 | docs/guides/shared_vs_handle.md | 判断フローが複雑 |
| 曖昧サフィックス禁止 | okite-ai + tech.md | 既存コードとの整合チェックなし |
| 参照実装との比較 | CLAUDE.md に記載のみ | 具体的な手順がない |

### 課題D: プロジェクト固有スキルが不在

`.claude/skills/` が完全に空。グローバルスキル（`clean-architecture`, `domain-building-blocks` 等）は汎用的だが、fraktor-rs 固有のワークフローをガイドするものがない。

---

## 2. 提案

### 提案1: ルール体系の3層化（推奨度: 高）

```
Tier 1: Dylint lint（機械的に強制・違反即エラー）
  → 現在の7つのlint + 今後追加可能

Tier 2: .claude/rules/（AIが必ず従うルール）
  → CLAUDE.md から抽出した判断フロー付きルール
  → 権威ある単一の場所

Tier 3: docs/guides/ + steering（参照ガイド）
  → 背景説明・設計思想・テンプレート
```

**具体的なアクション**:
- CLAUDE.md をスリム化し、詳細ルールは `.claude/rules/` へ移動
- okite-ai の5ルールのうち fraktor-rs に該当するものを `.claude/rules/` へ統合
- Serena memory の `style_and_conventions` を整理（重複排除）

### 提案2: プロジェクト固有スキルの新設（推奨度: 高）

3つのスキルを提案：

**a) `fraktor-new-module` スキル**
- トリガー: 「新しいモジュールを作りたい」「型を追加したい」
- 内容: no_std/std 分離パターン、1file1type の判断フロー（例外基準含む）、テスト配置、FQCN import パターンの実装テンプレート

**b) `fraktor-shared-design` スキル**
- トリガー: 「共有型を作りたい」「&mut self か &self か迷う」「Shared を新設したい」
- 内容: `docs/guides/shared_vs_handle.md` をスキル化し、判断フロー + テンプレートコードを提供

**c) `fraktor-type-review` スキル**
- トリガー: 「型の設計をレビューして」「過剰設計チェック」「YAGNI チェック」
- 内容: 公開型の必要性判断、参照実装との比較手順、曖昧サフィックスチェック、小型型の同居/分離判断

### 提案3: 「1 file = 1 public type」の例外基準を明文化（推奨度: 高）

Phase 1-4 の実績を踏まえた運用ルール案：

```
例外として同居を許可する条件（すべて満たすこと）：
1. 型が ≤20行 である
2. 関連する公開型と「同一概念の部品」である
   （例: TickDriverKind と AutoProfileKind）
3. 単独では再利用されない（他のモジュールから直接参照されない）
4. 同居先ファイルが 200行 を超えない

例外に該当しないケース：
- エラー型（独自のFrom実装やDisplay実装を持つため）
- Shared/Handle型（独自の責務を持つため）
- テスト対象となる型（テストファイルの紐づけが曖昧になるため）
```

**注意**: `type-per-file-lint` の挙動もこの例外基準に合わせて調整が必要。

### 提案4: 新 lint の検討（推奨度: 中）

| lint 候補 | 目的 | 複雑度 |
|-----------|------|--------|
| `ambiguous-suffix-lint` | Manager/Util/Facade等の禁止サフィックス検出 | 低 |
| `cqs-method-lint` | &self メソッドが状態変更していないか検出 | 高（断念案件かも） |

`ambiguous-suffix-lint` は効果対コスト比が良く、既存の okite-ai ルールを機械化できる。

---

## 3. 優先順位

| # | 提案 | 効果 | 工数 | 推奨 |
|---|------|------|------|------|
| 1 | 例外基準の明文化 | 高 | 小 | 最優先 |
| 2 | `fraktor-shared-design` スキル | 高 | 中 | 次に着手 |
| 3 | `fraktor-new-module` スキル | 高 | 中 | 同上 |
| 4 | ルール体系3層化（CLAUDE.md スリム化） | 中 | 中 | 後続 |
| 5 | `fraktor-type-review` スキル | 中 | 中 | 後続 |
| 6 | `ambiguous-suffix-lint` | 低 | 中 | 任意 |

---

## 4. 難しい課題の深掘り

### 4.1 ルールの権威ある場所が定まっていない

CLAUDE.md に書くか、rules に書くか、lint にするか、steering に書くかの判断基準がない。

**提案する判断基準**:
- **機械的に検証可能** → Dylint lint（Tier 1）
- **判断フローが必要だがAIは必ず従うべき** → `.claude/rules/`（Tier 2）
- **背景説明・設計思想・テンプレート** → `docs/guides/` + `.kiro/steering/`（Tier 3）
- **CLAUDE.md** → Tier 2 への参照ポインタ + lint/スキルでカバーされない最小限のルールのみ

### 4.2 スキルの粒度問題

汎用的すぎると役に立たず、具体的すぎるとメンテコストが高い。

**提案する粒度基準**:
- fraktor-rs 固有の「判断が難しいパターン」に絞る
  - `&mut self` vs `&self`（判断フローが複雑）
  - 型の同居/分離（例外基準が必要）
  - no_std/std 分離（テンプレートが有効）
- 汎用ルール（YAGNI, CQS 等）はスキル化せず、`.claude/rules/` に置く

### 4.3 lint と人間判断の境界

全部 lint にすると柔軟性が失われ、全部ルール記載だとAIが見落とす。

**提案する境界**:
- **lint**: 構文的に判定可能なもの（ファイル名、import順序、モジュール構造）
- **スキル**: 設計判断を伴うもの（型設計、共有パターン選択）
- **ルール**: lint/スキルの中間（曖昧サフィックス禁止は lint 化可能だが例外判断も必要）

---

## 5. ルール配置の整理案（ルール体系3層化の詳細）

### CLAUDE.md に残すもの（最小限）
- 言語ルール（日本語、rustdoc英語）
- 完了条件（テストパス、ci-check.sh all）
- 禁止事項の要約（lint allow禁止、CHANGELOG編集禁止）
- `.claude/rules/` と `docs/guides/` への参照ポインタ

### `.claude/rules/` に移動するもの（新設ファイル）
- `design-principles.md`: CQS、内部可変性、&mut self原則、YAGNI/Less is more
- `naming-conventions.md`: 曖昧サフィックス禁止、Shared/Handle命名
- `type-organization.md`: 1file1type + 例外基準、公開範囲の判断フロー
- `reference-implementation.md`: protoactor-go/pekko からの逆輸入手順

### 削除/統合対象
- `references/okite-ai/.agent/rules/` → fraktor-rs 該当分を `.claude/rules/` に統合
- Serena memory `style_and_conventions` → `.claude/rules/` との重複を排除し最小化

---

## 6. 次のステップ

1. この分析レポートをレビューし、提案の方向性を承認/修正
2. 承認された提案から順次実施
3. 実施後、`actor-module-overengineering-analysis.md` の Phase 5 チェックボックスを更新
