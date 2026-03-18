# 実装スコープ

## 対象
- `modules/actor/src/std/pattern/circuit_breaker/tests.rs` — FakeClock導入、`thread::sleep`を`clock.advance()`に置換、境界値テスト2件追加
- `modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs` — FakeClock導入、`tokio::time::sleep`を`clock.advance()`に置換
- `modules/actor/src/std/scheduler/tick/tests.rs` — `start_paused = true`に移行、`yield_now()`追加
- `modules/actor/src/std/system/base/tests.rs` — 不要な20ms sleep削除
- `modules/remote/src/std/endpoint_transport_bridge/tests.rs` — sleep使用テスト8件に`start_paused = true`追加、periodic テストに`yield_now()`追加
- `modules/remote/tests/quickstart.rs` — `start_paused = true`追加
- `modules/remote/tests/multi_node_scenario_integration.rs` — `start_paused = true`追加

## 非対象
- `modules/remote/src/std/endpoint_transport_bridge/tests.rs` 内の `inbound_handler_rejects_frames_when_queue_is_full` — `thread::sleep`によるバックプレッシャーシミュレーションが必要な実時間統合テストのため除外
- `modules/remote/src/std/transport/tokio_tcp/tests.rs` — 実transport契約テスト（計画のグループC）のため今回の単体テスト再設計対象外
- `modules/cluster/src/std/tokio_gossip_transport/tests.rs` — 実transport契約テスト（計画のグループC）のため今回の単体テスト再設計対象外
- `scripts/ci-check.sh` — CI分離はテストファイルではなくスクリプト変更であり、implementムーブメントで対応
- 各`Cargo.toml` — `test-util` feature追加はプロダクションコード変更に該当し、implementムーブメントで対応

## スコープ判断の理由
- 計画のグループA（単体へ寄せる対象）とグループB（統合のまま待ち方改善）のテストファイルを対象とした
- グループC（実時間統合テスト）は計画上「実時間依存として明示的に残す」対象であり、テスト構造変更の対象外
- `write_tests`ムーブメントの制約「テストファイルのみ作成可能」に従い、Cargo.toml・CI スクリプト等の非テストファイル変更はimplementムーブメントに委譲した
- 全7ファイルの変更は前回イテレーションで既に適用済みであることを確認。追加の変更は不要