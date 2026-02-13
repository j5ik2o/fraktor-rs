---
name: reviewing-fraktor-types
description: fraktor-rsのモジュール・型設計を「Less is more」「YAGNI」の観点でレビューし、過剰設計を検出する。参照実装（pekko/protoactor-go）との比較、定量分析、公開範囲の最適化を行う。トリガー：「型の設計をレビューして」「過剰設計チェック」「YAGNIチェック」「ファイル統合」「公開範囲を見直して」「type review」「overengineering check」等の型設計レビューリクエスト時に使用。
---

# fraktor-rs 型設計レビュー

モジュール・型の設計を「Less is more」「YAGNI」の観点でレビューし、過剰設計を検出・改善提案する。

## Workflow

### 1. 対象の特定

レビュー対象を確認する：

- **スコープ**: 特定モジュール / サブディレクトリ / クレート全体
- **動機**: 新規設計のレビュー / 既存コードのリファクタリング / 定期点検

### 2. 定量分析

対象ディレクトリに対して以下を計測する：

```bash
# ファイル数・行数（テストファイル除外）
find <target> -name "*.rs" ! -name "tests.rs" | wc -l
find <target> -name "*.rs" ! -name "tests.rs" -exec wc -l {} + | sort -n

# 30行以下のファイル数（基準値テーブルの「30行以下ファイル率」に対応）
find <target> -name "*.rs" ! -name "tests.rs" -exec wc -l {} + | awk '$1 <= 30 {count++} END {print count}'

# 公開型の数
grep -r "^pub struct\|^pub trait\|^pub enum" <target> --include="*.rs" | wc -l
```

**基準値**（Phase 1-4 の実績から導出）:

| 指標 | 正常 | 要注意 | 過剰 |
|------|------|--------|------|
| 30行以下ファイル率 | < 20% | 20-35% | > 35% |
| 平均行数/ファイル | > 80行 | 50-80行 | < 50行 |
| pub 型 vs pub(crate) 型比率 | 外部利用に応じた比率 | - | pub が大半 |

### 3. 参照実装との比較

対象と同等の機能を持つ参照実装の型数を確認する：

1. **pekko** (`references/pekko/`): Scala の対応クラス数を調査
2. **protoactor-go** (`references/protoactor-go/`): Go の対応型数を調査

```
fraktor-rs の型数 / 参照実装の型数 = 倍率

倍率 1-2x: 正常（Rust の所有権・ライフタイム要件で型が増えるのは自然）
倍率 2-3x: 要注意（no_std/std 分離や Shared 型を差し引いても多い可能性）
倍率 3x+:  過剰（設計見直し推奨。Phase 1-4 で message_adapter が 6.5x → 統合で改善した実績あり）
```

### 4. 問題パターン検出

以下のパターンを探す：

#### a) 薄い型の過剰分離
- 20行以下の enum / newtype が独立ファイルになっている
- 関連する親型と「同一概念の部品」であり、単独では再利用されない

#### b) 型の重複
- 同じフィールドを持つ複数の struct
- 同じバリアントを持つ複数の enum（例: 両方に TypeMismatch がある）
- Error 型と Failure 型の重複

#### c) 不要な公開
- `pub` だが crate 外から参照されていない型
- 再エクスポートされているが直接参照がない型

#### d) 過剰な抽象化
- trait の実装が1つしかない
- ジェネリクスが不要な場面でジェネリクスを使用

### 5. 改善提案

検出した問題に対して具体的な改善案を提示する：

| 問題 | 改善案 |
|------|--------|
| 薄い型の過剰分離 | 親型と同居（ただし同居先200行以下） |
| 型の重複 | 統合して単一型に |
| 不要な公開 | `pub(crate)` に変更 |
| 過剰な抽象化 | 具体型に簡素化 |

### 6. 公開範囲の最適化

`mcp__serena__find_referencing_symbols` で各公開型の参照元を調査し、以下に分類：

- **pub**: 外部クレートから参照される / 公開 API
- **pub(crate)**: crate 内部でのみ参照
- **非公開**: 特定モジュール内でのみ参照

## 使用例

### 例1: tick_driver モジュールのレビュー

**リクエスト**: 「tick_driver の型を見直したい」

**定量分析結果**:
- 29サブモジュール、10-20行のファイルが多数
- 30行以下ファイル率: 45%

**参照実装比較**:
- pekko: Scheduler 関連 5型 → fraktor-rs: 27公開型（5.4倍）

**検出パターン**:
- `TickDriverId(u64)`: 20行、独立ファイル → newtype は維持（ドメインプリミティブ）
- `HardwareKind`: 再エクスポートのみ、直接参照なし → 削除候補
- `TickMetricsMode`: 22行、`TickDriverConfig` でのみ使用 → pub(crate) 化候補

**改善提案**:
- 未使用の公開型6個を pub(crate) に変更
- 再エクスポートのみの型3個を整理

### 例2: 新規モジュールの事前レビュー

**リクエスト**: 「ClusterMembership の設計をレビューして」

**参照実装比較**:
- protoactor-go: cluster/membership に 4型
- 設計案: 12型

**判定**: 3倍超 → 設計の簡素化を推奨
- 内部状態型3個を pub(crate) に
- 設定enum2個を親型に同居

## 参照ドキュメント

- `claudedocs/actor-module-overengineering-analysis.md`: Phase 1-4 の分析基準と実績。基準値テーブルの根拠や過去の改善事例を確認する際に参照
- `references/pekko/`: Scala 参照実装。ステップ3 で対応クラス数を調査する際に参照
- `references/protoactor-go/`: Go 参照実装。ステップ3 で対応型数を調査する際に参照

## 出力ガイドライン

- レビュー結果は定量データと具体的な改善案をセットで提示
- 「ドメインプリミティブは統合しない」原則を尊重（TickDriverId 等の newtype は維持）
- 改善案は懸念度（高/中/低）で優先順位をつける
- 大規模な変更は Phase 分けして段階的に提案
