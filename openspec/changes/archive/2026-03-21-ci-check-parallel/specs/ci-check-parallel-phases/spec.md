## ADDED Requirements

### Requirement: run_all は3フェーズで実行される
`run_all()` は Phase 1（ゲート）→ Phase 2（lint並列）→ Phase 3（テスト並列）の順に実行しなければならない（SHALL）。各フェーズが失敗した場合、後続フェーズは実行してはならない。

#### Scenario: 正常系 - 全フェーズ成功
- **WHEN** `ci-check.sh all` を実行する
- **THEN** Phase 1（fmt, check_unit_sleep）が直列実行され、Phase 2（dylint, clippy, no-std, doc）が並列実行され、Phase 3（unit-test, integration-test）が並列実行される

#### Scenario: Phase 1 失敗時
- **WHEN** `fmt` がエラーを返す
- **THEN** Phase 2 および Phase 3 は実行されず、exit code 1 で終了する

#### Scenario: Phase 2 の1つが失敗
- **WHEN** Phase 2 の `clippy` が失敗し、他の `dylint`, `no-std`, `doc` は成功する
- **THEN** Phase 2 の全ジョブ完了を待ってからエラーを報告し、Phase 3 は実行されず、exit code 1 で終了する

### Requirement: 並列ジョブは別 CARGO_TARGET_DIR を使用する
並列実行される各ジョブは、独立した `CARGO_TARGET_DIR`（`target/ci-check/<shard>` 形式）を使用しなければならない（SHALL）。cargo のロック競合を防止する。

#### Scenario: clippy と dylint の並列実行
- **WHEN** Phase 2 で clippy と dylint が同時実行される
- **THEN** clippy は `target/ci-check/clippy` を、dylint は `target/ci-check/dylint` をターゲットディレクトリとして使用し、互いに干渉しない

### Requirement: 個別コマンド実行の動作は変更しない
`ci-check.sh clippy` や `ci-check.sh unit-test` 等の個別コマンド実行は、従来通り直列で実行されなければならない（SHALL）。並列化は `run_all` のみに適用される。

#### Scenario: 個別コマンドの直列実行
- **WHEN** `ci-check.sh clippy` を実行する
- **THEN** clippy のみが従来通りデフォルトの `CARGO_TARGET_DIR` で直列実行される

### Requirement: start_parallel_phase ヘルパーの新設
関数単位で並列実行するための `start_parallel_phase` ヘルパーを新設しなければならない（SHALL）。既存の `start_parallel_cargo`（単一 cargo コマンド用）を補完する。

#### Scenario: run_dylint の並列実行
- **WHEN** `start_parallel_phase "dylint" "dylint" run_dylint` を呼び出す
- **THEN** `run_dylint` がサブシェル内で `CARGO_TARGET_DIR=target/ci-check/dylint` を設定した状態でバックグラウンド実行される
