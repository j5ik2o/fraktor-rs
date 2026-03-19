## Context

機械的なモジュール構造の正規化。設計判断は不要。

## Goals / Non-Goals

**Goals:** std.rs のインラインモジュール定義を外部ファイルに抽出する

**Non-Goals:** ロジック変更、API 変更

## Decisions

core 側の既存パターンにそのまま従う。
