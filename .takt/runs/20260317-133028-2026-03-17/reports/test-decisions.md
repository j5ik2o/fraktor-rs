# 実装判断ログ

## 主要判断
| 項目 | 判断 | 理由 |
|------|------|------|
| CircuitBreaker の時間制御方式 | 非公開 `now_fn: impl Fn() -> Instant` 注入点を採用 | CircuitBreaker は同期型（`&mut self`）で `Instant::now()` を直接使用しており、tokio の `start_paused` では制御不可。公開 Clock trait は YAGNI に反するため非公開に限定 |
| FakeClock の設計 | base `Instant` + `Arc<AtomicU64>` オフセット方式を採用 | `Instant` は任意値を生成できないため、base からのオフセット加算で制御。`duration_since` が正しく動作し、決定的テストが可能 |
| CircuitBreakerShared の時間制御 | 内部 CircuitBreaker に clock を委譲する `new_with_clock` を採用 | Shared は CircuitBreaker のラッパーであり、clock 注入は内部の CircuitBreaker に委譲するのが自然 |
| Scheduler tick テストのランタイム | `current_thread, start_paused = true` に変更 | Tokio interval driver の検証には tokio 仮想時間制御が必要。`multi_thread` から `current_thread` に変更し `yield_now()` で spawned task の実行を保証 |
| system/base テストの sleep | 単純削除 | assert 対象（system_name）は構築時に決定済み。メッセージ処理完了の待機は不要 |
| endpoint_transport_bridge テストの時間制御 | `start_paused = true` を追加 | bridge 内部タイマーは tokio ベースのため仮想時間で制御可能。handshake timeout や periodic タスクを決定的に検証できる |
| `inbound_handler_rejects_frames_when_queue_is_full` テスト | `start_paused` 適用せず実時間統合テストとして維持 | TestTransport 内の `thread::sleep` による backpressure シミュレーションが `current_thread` + `start_paused` と競合するため |
| remote quickstart/multi_node テスト | `start_paused = true` を追加 | ManualTestDriver 構成で actor system 側の時間依存はなく、`tokio::time::sleep` の仮想時間化のみで十分 |
| Cargo.toml の `test-util` feature 追加 | implement ムーブメントに委譲 | write_tests ムーブメントの制約「テストファイルのみ変更可能」に該当しないため |
| テストファイルの追加変更要否 | 変更不要（前回イテレーションの成果を確認） | 全7ファイルの変更が既にワークツリーに適用済みであることを検証し、スコープとの一致を確認した |

## 影響とトレードオフ
- `CircuitBreaker::new_with_clock` と `CircuitBreakerShared::new_with_clock` は `#[cfg(test)]` かつ `pub(crate)` とする想定。公開 API への影響はゼロ
- `start_paused = true` 適用テストは tokio の `test-util` feature が必要。implement ムーブメントで `modules/actor/Cargo.toml` と `modules/remote/Cargo.toml` の dev-dependencies に `test-util` を追加するまでビルドエラーが発生する（想定内）
- scheduler tick テストを `multi_thread` から `current_thread` に変更したことで、実際のマルチスレッド環境での振る舞いは検証対象外になる。ただし検証対象は tokio interval のティック生成であり、スレッド間相互作用ではないため問題なし
- endpoint_transport_bridge の `thread::sleep`（TestTransport 内の遅延シミュレーション）は今回未変更。backpressure テスト以外では delay が 0 のため影響なし