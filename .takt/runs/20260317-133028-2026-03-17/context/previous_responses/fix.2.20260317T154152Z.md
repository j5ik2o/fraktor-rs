全1114テスト通過。

## 作業結果
- アーキテクトレビューの `ARCH-NEW-unit-timeout-check-missing` (spec-violation) を修正
- `cancel_during_half_open_records_failure` テストから `tokio::time::timeout` を除去し、`tokio::select!` + drop ベースのキャンセルに書き換え
- `ci-check.sh` の `check_unit_sleep` を `tokio::time::timeout` まで検査対象に拡張

## 変更内容
- `modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs`: `#[tokio::test]` → `#[tokio::test(start_paused = true)]`、`tokio::time::timeout` を `tokio::select!` + `yield_now` による明示的キャンセルに置換
- `scripts/ci-check.sh`: Phase 2 の検査パターンに `tokio::time::timeout` を追加、ログメッセージとエラーメッセージを更新

## ビルド結果
- `cargo test -p fraktor-actor-rs --lib --features test-support,std,tokio-executor` — コンパイル成功

## テスト結果
- `cargo test -p fraktor-actor-rs --lib --features test-support,std,tokio-executor -- pattern::circuit_breaker_shared::tests`: 10 passed, 0 failed
- `cargo test -p fraktor-actor-rs --lib --features test-support,std,tokio-executor`: 1114 passed, 0 failed
- `scripts/ci-check.sh check-unit-sleep`: 違反なし

## 収束ゲート
| 指標 | 件数 |
|------|------|
| new（今回修正） | 1 |
| reopened（再発修正） | 0 |
| persists（前回から継続、今回未着手） | 0 |

## 証拠
- `architect-review.md` の `ARCH-NEW-unit-timeout-check-missing` 指摘を確認
- `circuit_breaker_shared/tests.rs:160` にあった `tokio::time::timeout` を `tokio::select!` + drop に置換完了（grep で残存ゼロ確認）
- `ci-check.sh` Phase 2 パターンに `tokio::time::timeout` 追加済み（`rg -l 'tokio::time::sleep|tokio::time::timeout'`）
- `endpoint_transport_bridge/tests.rs:1136` の `tokio::time::timeout` は `start_paused = true` テスト内のため Phase 2 で許可される（既存テスト 1114 件全通過で回帰なし）
- `family_tag: spec-violation` の再発防止は `ci-check.sh check-unit-sleep` が `tokio::time::timeout` を検査対象にしたことで、今後 `start_paused` なしの unit テストに実時間 timeout が追加された場合に CI が失敗する