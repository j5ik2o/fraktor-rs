# 実装計画

## タスク形式テンプレート

作業分解に合う形式を使う:

### 主要タスクのみ
- [ ] {{NUMBER}}. {{TASK_DESCRIPTION}}{{PARALLEL_MARK}}
  - {{DETAIL_ITEM_1}} *(詳細が必要な場合だけ書く。単独で十分なタスクではdetail bulletを省略する。)*
  - _Requirements: {{REQUIREMENT_IDS}}_

### 主要タスク + サブタスク構造
- [ ] {{MAJOR_NUMBER}}. {{MAJOR_TASK_SUMMARY}}
- [ ] {{MAJOR_NUMBER}}.{{SUB_NUMBER}} {{SUB_TASK_DESCRIPTION}}{{SUB_PARALLEL_MARK}}
  - {{DETAIL_ITEM_1}}
  - {{DETAIL_ITEM_2}}
  - {{OBSERVABLE_COMPLETION_ITEM}} *(少なくとも1つのdetail itemで、このタスクの観測可能な完了条件を書く。)*
  - _Requirements: {{REQUIREMENT_IDS}}_ *(IDだけを書く。説明や括弧書きを追加しない。)*
  - _Boundary: {{COMPONENT_NAMES}}_ *((P) タスクでは必須。scopeが明らかな場合は省略する。)*
  - _Depends: {{TASK_IDS}}_ *(非自明な境界横断依存がある場合だけ書く。多くのタスクでは省略する。)*

> **並列マーカー**: 並列実行可能なタスクにだけ ` (P)` を付ける。`--sequential` modeでは省略する。
>
> **任意のテストカバレッジ**: サブタスクが受け入れ条件に紐づく延期可能なテスト作業である場合だけ、checkboxを `- [ ]*` にし、参照する要件をdetail bulletで説明する。
