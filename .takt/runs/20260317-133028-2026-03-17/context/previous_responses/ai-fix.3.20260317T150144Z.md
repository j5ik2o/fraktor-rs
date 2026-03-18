## AI修正完了

`persists` 状態の2件の指摘をすべて修正しました。

### ai-review-f001 (HIGH) — 修正実施
- **`ci-check.sh`**: glob パターン `*/tests.rs` → `**/tests.rs`、`*/tests/*.rs` → `**/tests/*.rs` に修正。深いパスの sleep 使用を正しく検出可能に
- **`circuit_breaker_shared/tests.rs`**: `tokio::time::sleep(Duration::from_secs(60)).await` → `std::future::pending::<()>().await` に置換（キャンセルテストには永遠に完了しない Future で十分）
- glob 修正で新たに検出された既存の sleep（`coordinated_shutdown/tests.rs`、`dispatcher/tests.rs`）はタスクスコープ外のため allowlist に追加

### ai-review-f003 (MEDIUM) — 修正実施
- `circuit_breaker_shared.rs` L72, L77 の What/How コメント（RAII ガード説明、disarm 説明）を削除
- `circuit_breaker_shared/tests.rs` L163 のシミュレーション説明コメントもコード置換に伴い削除

### テスト結果
- circuit_breaker_shared テスト: 10 passed
- actor 全体テスト: 1114 passed
- ci-check unit sleep 検査: 違反なし