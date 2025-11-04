## 1. カバレッジ設計
- [x] 1.1 `specs/001-add-actor-runtime` のユーザーストーリー/シナリオを洗い出し、既存テストとのギャップ表を作成する。
- [x] 1.2 Mailbox/Dispatcher/Supervisor/EventStream/Ask/Deadletter のそれぞれで必要となる前提データと検証メトリクス（ログ、カウンタ、Future）を ActorSystem + TokioExecutor 前提で定義する。

## 2. テスト基盤整備
- [x] 2.1 Tokio ランタイムを初期化して `DispatcherConfig::from_executor(ArcShared::new(TokioExecutor::new(handle)))` を差し込むフィクスチャ、有限待機ヘルパー、イベント記録ユーティリティを `tests/common.rs` 相当へ集約する。
- [x] 2.2 `modules/actor-std/examples/ping_pong_tokio` をベースに、受入テストから再利用可能な Props/Guardian/Executor 構築ラッパーを整理し、Tokio ランタイム設定（スレッド数や `spawn_blocking`）を明文化する。

## 3. 受入テスト追加
- [x] 3.1 US1: spawn/tell/ask/バックプレッシャー/スループット（TokioExecutor）を検証する受入テストを `ping_pong_tokio.rs` 系列に追加し、reply_to / throughput300 と Tokio のスレッドプール設定が仕様通り動くか確認する。
- [x] 3.2 US2: 再起動戦略、エスカレーション、親停止伝播を `supervisor.rs` 系列で網羅し、`restart_statistics` や Deadletter 入力を検証する。
- [x] 3.3 US3: EventStream/Deadletter/Lifecycle/Log/Mailbox メトリクス通知を `event_stream.rs` / `system_events.rs` 等に追加し、PID・タイムスタンプ妥当性やサブスクライブ API の解除タイミングを確認する。

## 4. 検証
- [x] 4.1 `cargo test -p cellactor-actor-std-rs --tests --features tokio-executor` と `cargo test -p cellactor-actor-core-rs --tests`（必要なら `--features alloc-metrics` 等）を実行し、TokioExecutor ベースと core/Inline 両方の受入テストが CI で通過することを確認する。
- [x] 4.2 `./scripts/ci-check.sh all` を走らせ、lint/format/benchdry-run を含めてパスすることを確認する。
- [x] 4.3 主要テストのログ出力やメトリクス数値を `docs/` かテストコメントに記録し、再現手順を残す。
