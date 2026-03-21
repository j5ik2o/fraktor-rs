# 決定ログ

## 1. `SubstreamCancelStrategy` は独立した core の設定型として追加した
- **背景**: `Flow::group_by` / `Source::group_by` の新契約では、`Drain` / `Propagate` を公開 API から必須指定する必要がある。
- **検討した選択肢**: `group_by` 実装ファイル内に閉じた型として置く / core 直下の公開設定型として追加して export する
- **理由**: 既存テストは `crate::core::SubstreamCancelStrategy` を参照しており、利用側契約に合わせるには core の公開型として定義するのが最小差分だったため。

## 2. `group_by` の内部設定保持は `GroupByLogic` に閉じた
- **背景**: 今回の変更範囲は `group_by` の定義・引数受け渡し・ステージ構築に限定され、実行系の振る舞い変更は対象外。
- **検討した選択肢**: 実行系まで踏み込んでキャンセル動作を変える / 現時点では `GroupByLogic` に設定を保持して公開 API とステージ構築だけを整合させる
- **理由**: 指示どおり変更範囲を `group_by` の契約面と内部保持値に限定し、後続ムーブメントで扱うべき実行系変更へ踏み込まないため。

## 3. `max_substreams = 0` の異常系は既存バリデーションを維持した
- **背景**: `Flow::group_by` / `Source::group_by` はすでに `validate_positive_argument("max_substreams", max_substreams)` でゼロを拒否している。
- **検討した選択肢**: 新契約導入に合わせて異常系を作り直す / 既存バリデーションをそのまま維持する
- **理由**: 既存契約の維持が明示要求であり、公開 API の異常系は既存実装で十分に満たせるため。

## 実行結果
- `cargo check -p fraktor-stream-rs --tests --features std` → 成功
- `cargo test -p fraktor-stream-rs --lib --features std group_by_` → 成功（12 passed）
