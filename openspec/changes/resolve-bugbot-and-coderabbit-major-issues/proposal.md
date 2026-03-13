# BugBot と CodeRabbit major 指摘の解消

## 背景

Open な `[BugBot]` と `[CodeRabbit:major]` issue には、actor runtime の安全性、streams の backpressure 契約、`.takt`/CI 設定の整合を壊すものが複数残っています。個別に場当たり対応すると不変条件が再び崩れるため、関連 issue をまとめて根本原因単位で解消する change が必要です。

## 変更内容

- actor モジュールで mailbox、dispatcher、supervision、typed behavior、stash、router 周辺の安全性と不変条件を修正する
- streams モジュールで source queue、actor sink、timer/apply failure、`Source::create` の backpressure と終端契約を修正する
- `.takt` の piece / instruction / output contract を修正し、壊れたテンプレート構造や未参照 instruction を解消する
- `scripts/ci-check.sh` の `cargo` 実行経路を統一し、AI ガードがすべての実行経路に適用されるようにする
- major / bug 指摘をテストで再発防止できるよう、対象モジュールの回帰テストを補強する

## ケイパビリティ

### 新規ケイパビリティ

- `actor-runtime-safety`: actor runtime が mailbox policy、dispatcher 選択、supervision 再起動、stash 操作、router 挙動の不整合を起こさないことを定義する
- `streams-backpressure-integrity`: streams が backpressure、future wake、timer/apply failure、terminal 状態遷移を一貫して扱うことを定義する
- `workflow-integrity`: `.takt` と CI スクリプトが有効な構造と一貫した実行経路を保つことを定義する

### 既存ケイパビリティへの変更

<!-- 既存 spec はまだ存在しないため空 -->

## 影響

- 影響範囲:
  `modules/actor/src/core/**`,
  `modules/actor/examples/**`,
  `modules/streams/src/core/**`,
  `modules/streams/src/std/**`,
  `.takt/**`,
  `scripts/ci-check.sh`
- 影響する挙動:
  mailbox policy と queue の整合、bounded queue の同期、supervised restart、group router hash/契約、source queue/actor sink/backpressure、OpenSpec/TAKT artifact と CI 実行経路の構造整合
- 検証方法:
  actor / streams の対象テスト、`.takt` 構造確認、`./scripts/ci-check.sh ai all`
