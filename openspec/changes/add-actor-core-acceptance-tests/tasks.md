## 1. カバレッジ設計
- [ ] 1.1 `specs/001-add-actor-runtime` のユーザーストーリー/シナリオを洗い出し、既存テストとのギャップ表を作成する。
- [ ] 1.2 Mailbox/Dispatcher/Supervisor/EventStream/Ask/Deadletter のそれぞれで必要となる前提データと検証メトリクス（ログ、カウンタ、Future）を ActorSystem（NoStdToolbox）前提で定義する。

## 2. テスト基盤整備
- [ ] 2.1 `modules/actor-core/tests` で共有できるフィクスチャ／ユーティリティ（カスタムメッセージ型、有限待機ヘルパーなど）を設計し、`tests/common.rs` のような単一ファイルで再利用できる形にまとめる。
- [ ] 2.2 `ping_pong_no_std` を参考に ActorSystem（NoStdToolbox）で再利用可能なテスト用 Props/Guardian を整理する。

## 3. 受入テスト追加
- [ ] 3.1 US1: spawn/tell/ask/バックプレッシャー/スループット（Inline Executor）を検証する受入テストを `ping_pong.rs` 系列に追加し、reply_to / throughput300 の仕様を確認する。
- [ ] 3.2 US2: 再起動戦略、エスカレーション、親停止伝播を `supervisor.rs` 系列で網羅し、`restart_statistics` や Deadletter 入力を検証する。
- [ ] 3.3 US3: EventStream/Deadletter/Lifecycle/Log/Mailbox メトリクス通知を `event_stream.rs` / `system_events.rs` 等に追加し、PID・タイムスタンプ妥当性やサブスクライブ API の解除タイミングを確認する。

## 4. 検証
- [ ] 4.1 `cargo test -p cellactor-actor-core-rs --tests`（必要なら `--features alloc-metrics` 等）と no_std 用ターゲット（`cargo test --target thumbv7em-none-eabihf` 等）でテストを実行し、ActorSystem 前提の受入テストが CI で通過することを確認する。
- [ ] 4.2 `./scripts/ci-check.sh all` を走らせ、lint/format/benchdry-run を含めてパスすることを確認する。
- [ ] 4.3 主要テストのログ出力やメトリクス数値を `docs/` かテストコメントに記録し、再現手順を残す。
