## Tasks

### Phase 1: インフラ構築

- [ ] **T1: showcases-std クレートの骨格作成**
  - `showcases-std/Cargo.toml` を作成（basic 依存 + `advanced` feature で重い依存を分離、`publish = false`）
  - `showcases-std/src/lib.rs` を作成（`pub mod support` を公開）
  - `showcases-std/support/mod.rs` と `showcases-std/support/tick_driver.rs` を作成
  - tick_driver_support を std 版ベースで統合（`Arc<Mutex>` ベース、1ファイル）
  - ルート `Cargo.toml` の `[workspace]` members に `"showcases-std"` を追加
  - `cargo build -p fraktor-showcases-std` でビルドが通ることを確認

### Phase 2: Basic examples — actor 系（7個、typed API）

- [ ] **T2: getting_started example 作成**
  - ActorSystem 起動 → guardian spawn → tell でメッセージ送信
  - 既存 `behaviors_setup_receive_std` を参考に、最小構成で書き直し

- [ ] **T3: request_reply example 作成**
  - ask パターンで応答を受け取る
  - 既存 `ask_typed_no_std` を参考に、std 版として書き直し

- [ ] **T4: state_management example 作成**
  - カウンターアクター、Behavior 切替で状態遷移
  - 既存 `behaviors_counter_typed_std` + `behaviors_state_transition_typed_no_std` を統合

- [ ] **T5: child_lifecycle example 作成**
  - 子アクター spawn、watch、Terminated シグナル、supervision
  - 既存 `death_watch_std` + `behaviors_receive_signal_std` + `supervision_std` の要素を1つに統合

- [ ] **T6: timers example 作成**
  - once（遅延実行）、periodic（定期実行）、cancellation を1つの example で
  - 既存 `scheduler_once_tokio_std` + `scheduler_periodic_typed_no_std` + `scheduler_cancellation_typed_no_std` を統合

- [ ] **T7: routing example 作成**
  - Pool Router による round-robin 負荷分散
  - 既存 `pool_router_round_robin_no_std` を std 版として書き直し

- [ ] **T8: stash example 作成**
  - メッセージの一時退避と復帰
  - 既存 `stash_unstash_typed_no_std` を std 版として書き直し

### Phase 3: Basic examples — serialization + stream（2個）

- [ ] **T9: serialization example 作成**
  - serde_json または bincode によるメッセージシリアライゼーション
  - 既存 `serialization_json_std` + `serialization_bincode_no_std` の要素を1つに統合

- [ ] **T10: stream_pipeline example 作成**
  - Source → Map → Fold → Sink のデータパイプライン
  - 既存 `map_filter_std` + `fold_aggregation_std` + `source_sink_minimal_std` を統合

### Phase 4: Advanced examples — cross-module（3個、現行 untyped API）

- [ ] **T11: persistent_actor example 作成**
  - イベントソーシングによるアクター状態永続化
  - 既存 `persistent_counter_no_std` を std 版として書き直し（untyped core API をそのまま使用）

- [ ] **T12: cluster_membership example 作成**
  - クラスタへの参加とメンバーシップ変更の観測
  - 既存 `cluster_extension_tokio` + `membership_gossip_tokio` の要素を統合（untyped core + std API）
  - `required-features = ["advanced"]`

- [ ] **T13: remote_messaging example 作成**
  - ネットワーク越しのアクター通信
  - 既存 `loopback_quickstart` + `tokio_tcp_quickstart` の要素を統合（untyped core + std API）
  - `required-features = ["advanced"]`

### Phase 5: ドキュメント・CI 参照更新

- [ ] **T14: README.md の更新**
  - `README.md` 行90付近の `modules/*/examples/` 参照を `showcases-std/` に更新
  - example の実行コマンドを `cargo run -p fraktor-showcases-std --example <name>` に統一

- [ ] **T15: docs/guides/getting-started.md の更新**
  - 行226, 233, 246 の旧 example パス（`modules/actor/examples/...`）を新パスに更新
  - 行299-309 の examples テーブルを新構成（12 examples）に更新
  - no_std quickstart の記述が新構成と整合するよう調整

- [ ] **T16: scripts/ci-check.sh の確認・修正**
  - `run_examples()` が `fraktor-showcases-std` の basic / advanced example を正しく認識・実行することを確認（`run_examples()` は metadata の `required-features` を読んで自動で `--features` を付与するため、基本的に追加対応は不要）
  - `integration-test` コマンド（行1049-1054）は現在 `--features test-support` 固定。advanced examples を `integration-test` の対象に含めるかどうかを判断し、含める場合は feature 指定を追加する

### Phase 6: クリーンアップ

- [ ] **T17: 既存 examples の削除**
  - `modules/actor/examples/` 内の全ディレクトリ・ファイルを削除
  - `modules/cluster/examples/` 内の全ディレクトリ・ファイルを削除
  - `modules/stream/examples/` 内の全ディレクトリ・ファイルを削除
  - `modules/persistence/examples/` 内の全ディレクトリ・ファイルを削除
  - `modules/remote/examples/` 内の全ディレクトリ・ファイルを削除
  - `modules/utils/examples/` 内の全ファイルを削除
  - 各モジュールの `Cargo.toml` から `[[example]]` セクションを全て削除

- [ ] **T18: 最終 CI 確認**
  - `cargo build -p fraktor-showcases-std --examples` で basic examples がビルドできること
  - `cargo build -p fraktor-showcases-std --features advanced --examples` で advanced examples がビルドできること
  - `./scripts/ci-check.sh ai all` が通ること
  - dylint lint が showcases-std クレートに不適切に適用されていないことを確認

## 依存関係

```
T1 (インフラ)
├── T2〜T10 (basic examples、並行可能)
│   ※ ただし support の変更が入った場合は影響を受ける example の再確認が必要
├── T11〜T13 (advanced examples、並行可能)
│   └── T14〜T16 (ドキュメント・CI 更新: 全 example 完了後)
│       └── T17 (既存削除: 参照更新完了後)
│           └── T18 (最終 CI 確認: 削除完了後)
```

## 実装上の注意

- 各 example の作成時に既存実装を読んでパターンを抽出してから書き直すこと（learning-before-coding ルール）
- `support/tick_driver.rs` は既存の `std_tick_driver_support.rs` をベースとし、不要な抽象化を除去
- Basic examples は `cargo run -p fraktor-showcases-std --example <name>` で動作確認すること
- Advanced examples は `cargo run -p fraktor-showcases-std --features advanced --example <name>` で動作確認すること
- Phase 6 の既存削除は全 example の動作確認 **および** ドキュメント・CI の参照更新が完了してから実施（レガシーコード一時許容ルール）
- Advanced examples（persistence, remote, cluster）は現行の untyped core API を使用する。将来 typed API が整備された時点で別タスクとして typed 化する
