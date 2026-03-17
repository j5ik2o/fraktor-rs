## arch-review
前回の `unit-timeout` 指摘が本当に解消したかを、`ci-check.sh` と `circuit_breaker_shared` の現物で再確認します。今回はその修正点に絞って、前回 `ARCH-NEW-unit-timeout-check-missing` を `resolved / persists` で判定し、あわせて変更ファイル内の新規設計問題がないかを拾います。
前回の open finding は 1 件だけなので、ここを潰せていれば基本は承認寄りです。`scripts/ci-check.sh` と `circuit_breaker_shared/tests.rs` を先に見て、そのうえで変更ファイル内に新しい構造問題が混ざっていないかだけ短く横断します。
前回の open finding は 1 件で、今回は `resolved` です。

- `finding_id: ARCH-NEW-unit-timeout-check-missing` `status: resolved`  
  [scripts/ci-check.sh:998](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/scripts/ci-check.sh#L998) の `check_unit_sleep` は [scripts/ci-check.sh:1033](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/scripts/ci-check.sh#L1033) で `tokio::time::timeout` まで検査対象に拡張され、[order.md:67](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/.takt/runs/20260317-133028-2026-03-17/context/task/order.md#L67) の `sleep / 実時間 timeout` 禁止に沿う形になりました。加えて、前回の実違反だった [modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs:147](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs#L147) は `start_paused = true` に変わり、[modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs:161](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs#L161) 以降も `tokio::select!` + drop ベースのキャンセルに置き換わっており、wall-clock timeout 依存は消えています。

今回の差分について、構造・公開範囲・変更ファイル内のデッドコード・テスト階層の分離を見直しましたが、`new` / `persists` のブロッキング問題は見つかりませんでした。`APPROVE` です。

このムーブメントではビルドやテストの再実行はしていません。

---

## supervise
architect-review.md の REJECT 指摘を実コードで照合しました。指摘された2点（`tokio::time::timeout` のCI検査対象拡張、テストの `start_paused` 化）はいずれも修正済みです。

---

# 最終検証結果

## 結果: APPROVE

## 要件充足チェック

| # | 要件（タスク指示書から抽出） | 充足 | 根拠（ファイル:行） |
|---|---------------------------|------|-------------------|
| 1 | 単体テスト層に `thread::sleep` / `tokio::time::sleep` / 壁時計待ちを残さない | ✅ | `check-unit-sleep` 実行→違反なし。`circuit_breaker_shared/tests.rs` に `tokio::time::timeout` ゼロ（grep確認） |
| 2 | 時間依存ロジックは fake/manual time で検証できる | ✅ | `circuit_breaker.rs:77` `new_with_clock`、`tick/tests.rs:23,59,89` `start_paused=true` + `advance` |
| 3 | 実時間依存テストは統合テスト層へ分離、CI で実行経路を分ける | ✅ | `ci-check.sh:983-996` unit/integration分離 |
| 4 | `ci-check` default 実行で長時間テストがボトルネックにならない | ✅ | `ci-check.sh:1128-1131` unit-first |
| 5 | sleep禁止の自動検査（allowlist方式） | ✅ | `ci-check.sh:998-1054` `check_unit_sleep`: Phase 1 `thread::sleep` 常時禁止、Phase 2 `tokio::time::{sleep,timeout}` を `start_paused` なしファイルで禁止 |
| 6 | `check_unit_sleep` が全対象モジュールを走査 | ✅ | `ci-check.sh:1004-1009` scan_dirs に `actor/src/`, `streams/src/`, `remote/src/`, `cluster/src/` |
| 7 | `cancel_during_half_open_records_failure` が実時間 timeout 不使用 | ✅ | `circuit_breaker_shared/tests.rs:147` `start_paused=true`、`:161-168` `tokio::select!` + `yield_now` ベースキャンセル |
| 8 | `new_with_clock` は `pub(crate)` | ✅ | `circuit_breaker.rs:77`、`circuit_breaker_shared.rs:42` |
| 9 | `tokio/test-util` が dev-dependencies に追加 | ✅ | `actor/Cargo.toml`、`remote/Cargo.toml` |

## 検証サマリー

| 項目 | 状態 | 確認方法 |
|------|------|---------|
| テスト（circuit_breaker_shared） | ✅ | 10 passed |
| テスト（actor lib全体） | ✅ | 1114 passed（前回fix確認済み） |
| CI sleep/timeout検査 | ✅ | `check-unit-sleep` → 違反なし |
| architect-review指摘対応 | ✅ | ARCH-NEW-unit-timeout-check-missing 修正完了（実コード照合済み） |

## 解消済み（resolved）

| finding_id | 解消根拠 |
|------------|----------|
| ARCH-NEW-unit-timeout-check-missing | `circuit_breaker_shared/tests.rs:147` `start_paused=true` 化、`:161-168` `tokio::select!` + `yield_now`。`ci-check.sh:1035` Phase 2 に `tokio::time::timeout` 追加。grep で `tokio::time::timeout` 残存ゼロ確認。`check-unit-sleep` パス |

## 成果物

前回までの全成果物に加え：
- 変更: `modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs` — `start_paused=true`、`tokio::select!` ベースキャンセルに書換え
- 変更: `scripts/ci-check.sh` — Phase 2 検査パターンに `tokio::time::timeout` 追加

---

# タスク完了サマリー

## タスク
単体テストから実時間依存（`thread::sleep` / `tokio::time::sleep` / `tokio::time::timeout`）を排除し、fake clock / 仮想時間で検証する構成へ移行。CI を unit / integration に分離し、sleep/timeout 使用の自動検査を追加。

## 結果
完了

## 変更内容

| 種別 | ファイル | 概要 |
|------|---------|------|
| 変更 | `modules/actor/src/std/pattern/circuit_breaker.rs` | clock 注入点（`pub(crate) new_with_clock`）追加 |
| 変更 | `modules/actor/src/std/pattern/circuit_breaker_shared.rs` | clock 注入の委譲 |
| 変更 | `modules/actor/src/std/pattern/circuit_breaker/tests.rs` | FakeClock ベーステスト |
| 変更 | `modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs` | FakeClock + `start_paused` + `select!` キャンセル |
| 変更 | `modules/actor/src/std/scheduler/tick/tests.rs` | `start_paused` + `advance` + `yield_now` |
| 変更 | `modules/actor/src/std/system/base/tests.rs` | 不要 sleep 削除 |
| 変更 | `modules/actor/Cargo.toml` | tokio `test-util` feature 追加 |
| 変更 | `modules/remote/Cargo.toml` | tokio `test-util` feature 追加 |
| 変更 | `modules/remote/src/std/endpoint_transport_bridge/tests.rs` | `start_paused` 追加、`thread::sleep` 除去 |
| 変更 | `modules/remote/tests/quickstart.rs` | `start_paused` 追加 |
| 変更 | `modules/remote/tests/multi_node_scenario_integration.rs` | `start_paused` 追加 |
| 変更 | `scripts/ci-check.sh` | unit/integration 分離、`check-unit-sleep` 全モジュール対応、Phase 1/2 検査（sleep + timeout） |

## 確認コマンド

```bash
cargo test -p fraktor-actor-rs --lib --features test-support,std,tokio-executor
cargo test -p fraktor-remote-rs --lib --features test-support,std,tokio-executor
./scripts/ci-check.sh ai check-unit-sleep
```