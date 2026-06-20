# Domain Docs

Engineering skills がこのリポジトリのドメイン文書を読むときのルール。

## Before exploring, read these

- root の `CONTEXT.md`
- `docs/adr/` のうち、作業対象に関係する ADR

このリポジトリは single-context repo として扱う。root に `CONTEXT-MAP.md` が追加された場合は、multi-context repo として、関連する context ごとの `CONTEXT.md` も読む。

対象ファイルが存在しない場合は静かに続行する。欠落を理由に upfront で新規作成を提案しない。`/domain-modeling` skill は、用語や判断が実際に解決された時点で必要な文書を遅延作成する。

## File structure

```text
/
├── CONTEXT.md
├── docs/adr/
│   └── 0001-failure-detector-configuration-contract.md
└── modules/
```

## Use the glossary's vocabulary

issue title、refactor proposal、hypothesis、test name などでドメイン概念を名付ける場合は、`CONTEXT.md` に定義された用語を使う。`CONTEXT.md` の `_Avoid_` にある言い換えへ drift させない。

必要な概念が glossary に無い場合は、既存語彙で表現できるかを先に確認する。新しい重要概念として確定する場合は、実装や spec へ進む前に `CONTEXT.md` への反映を検討する。

## Flag ADR conflicts

出力が既存 ADR と矛盾する場合は、黙って上書きせず明示する。

例:

> ADR-0001 と矛盾する可能性がある。Failure Detector Configuration (故障検出器設定) を Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) として扱わない前提を再確認する。
