## 実装タスクリスト

### Phase 1: コアインフラ構築（破壊的変更）

#### LifecycleEvent拡張
- [ ] `modules/actor-core/src/lifecycle/lifecycle_event.rs` - `watcher: Option<Pid>`フィールド追加
  - [ ] 構造体に`watcher: Option<Pid>`フィールド追加
  - [ ] `new_started(pid, parent, name, timestamp)` 便利メソッド実装
  - [ ] `new_restarted(pid, parent, name, timestamp)` 便利メソッド実装
  - [ ] `new_stopped(pid, parent, name, timestamp)` 便利メソッド実装
  - [ ] `new_terminated(pid, parent, name, timestamp, watcher)` 便利メソッド実装
  - [ ] `is_watched()` メソッド実装
  - [ ] `watcher()` getterメソッド実装
  - [ ] 単体テスト追加

#### SystemMessage拡張
- [ ] `modules/actor-core/src/messaging/system_message.rs` - Watch/Unwatch variant追加
  - [ ] `Watch(Pid)` variant追加
  - [ ] `Unwatch(Pid)` variant追加
  - [ ] Debugトレイト自動導出の確認
  - [ ] 単体テスト追加

#### 既存コード移行
- [ ] 既存のLifecycleEvent生成箇所を便利メソッドに移行
  - [ ] `modules/actor-core/src/actor_prim/actor_cell.rs` のLifecycleEvent生成箇所
  - [ ] `modules/actor-core/src/system/system_state.rs` のLifecycleEvent生成箇所
  - [ ] テストコードのLifecycleEvent生成箇所
  - [ ] コンパイルエラー箇所の修正

#### 検証
- [ ] `cargo test --package cellactor-core` が全てパス
- [ ] 破壊的変更が適切に処理されていることを確認

### Phase 2: ActorCell拡張

#### ActorCellフィールド追加
- [ ] `modules/actor-core/src/actor_prim/actor_cell.rs` - watchersフィールド追加
  - [ ] `watchers: ToolboxMutex<Vec<Pid>, TB>` フィールド追加
  - [ ] `new()` メソッドでwatchersを初期化
  - [ ] `handle_watch(watcher: Pid)` メソッド実装（冪等性保証）
  - [ ] `handle_unwatch(watcher: Pid)` メソッド実装
  - [ ] `notify_watchers_on_stop()` メソッド実装
    - [ ] 各監視者向けに`LifecycleEvent::new_terminated()`発行
    - [ ] システムワイド用に`LifecycleEvent::new_stopped()`発行
  - [ ] `stop()` メソッドに`notify_watchers_on_stop()`呼び出し追加
  - [ ] `stop()` メソッドでwatchersリストをクリア

#### 単体テスト
- [ ] watchersリストの追加・削除テスト
- [ ] 冪等性テスト（同じPidを複数回watch）
- [ ] 停止時のイベント発行テスト
- [ ] watchersリストのクリアテスト

#### 検証
- [ ] `cargo test --package cellactor-core` が全てパス
- [ ] ActorCellの単体テストが全て成功

### Phase 3: API追加

#### ActorContext拡張
- [ ] `modules/actor-core/src/actor_prim/actor_context.rs` - watch/unwatch API追加
  - [ ] `watch(target: &ActorRefGeneric<TB>)` メソッド実装
    - [ ] SystemMessage::Watch(self_pid)を送信
    - [ ] エラーハンドリング
    - [ ] docコメント追加
  - [ ] `unwatch(target: &ActorRefGeneric<TB>)` メソッド実装
    - [ ] SystemMessage::Unwatch(self_pid)を送信
    - [ ] エラーハンドリング
    - [ ] docコメント追加
  - [ ] `spawn_child_watched(props: &PropsGeneric<TB>)` 便利メソッド実装
    - [ ] spawn_childを呼び出し
    - [ ] 自動的にwatchを呼び出し
    - [ ] エラーハンドリング
    - [ ] docコメント追加

#### Actorトレイト拡張
- [ ] `modules/actor-core/src/actor_prim/actor.rs` - on_terminated追加
  - [ ] `on_terminated(ctx: &mut ActorContext, terminated: Pid)` デフォルト実装追加
  - [ ] デフォルト実装は`Ok(())`を返す
  - [ ] docコメント追加（使用例を含む）

#### 検証
- [ ] `cargo test --package cellactor-core` が全てパス
- [ ] APIドキュメントが正しく生成される

### Phase 4: SystemState統合

#### SystemMessage処理追加
- [ ] `modules/actor-core/src/system/system_state.rs` - Watch/Unwatch処理追加
  - [ ] `process_system_message()` にWatch処理を追加
    - [ ] ActorCellの`handle_watch()`呼び出し
    - [ ] エラーハンドリング
  - [ ] `process_system_message()` にUnwatch処理を追加
    - [ ] ActorCellの`handle_unwatch()`呼び出し
    - [ ] エラーハンドリング

#### EventStream統合
- [ ] EventStreamサブスクライバーでLifecycleEventのwatcherフィールドを処理
  - [ ] `watcher: Some(pid)`の場合、該当アクターの`on_terminated`を呼び出す仕組み実装
  - [ ] ActorCellでのLifecycleEventハンドリング実装

#### 検証
- [ ] `cargo test --package cellactor-core` が全てパス
- [ ] SystemMessageの処理が正常に動作

### Phase 5: テストと検証

#### 統合テスト作成
- [ ] `modules/actor-core/tests/death_watch.rs` 新規作成
  - [ ] 基本的な監視テスト（watch → 子停止 → on_terminated呼び出し）
  - [ ] unwatch後は通知されないテスト
  - [ ] 複数監視者が全員通知を受け取るテスト
  - [ ] システムワイド観測イベントも発行されるテスト
  - [ ] 監視者向けと通常イベントの区別テスト（`is_watched()`）
  - [ ] 循環監視でもデッドロックしないテスト
  - [ ] 既に停止したアクターをwatchしてもエラーにならないテスト
  - [ ] 同じアクターを複数回watchしても冪等テスト
  - [ ] `spawn_child_watched`の動作テスト

#### no_std環境テスト
- [ ] actor-coreのno_std環境でのビルド確認
- [ ] 全機能がno_std環境で動作することを確認

#### actor-std環境テスト
- [ ] actor-stdで同じAPIが利用可能であることを確認
- [ ] actor-stdのテストが全てパス
- [ ] `cargo test --package cellactor-std` が全てパス

#### 全体テスト
- [ ] `cargo test --workspace` が全てパス
- [ ] `cargo clippy --workspace` が警告なし
- [ ] `cargo fmt --check` がパス
- [ ] カバレッジ ≥ 90% を確認

#### 検証
- [ ] 全ての成功基準を満たす
- [ ] パフォーマンステスト（メモリ使用量、イベント発行数）
- [ ] エッジケース処理の確認

### Phase 6: ドキュメントと例

#### サンプルコード作成
- [ ] `modules/actor-std/examples/death_watch.rs` 新規作成
  - [ ] 基本的なwatch/unwatchの使用例
  - [ ] 子アクターの再起動パターン
  - [ ] `spawn_child_watched`の使用例
  - [ ] 複数の子アクターを監視する例

#### ドキュメント更新
- [ ] `README.md` にwatch/unwatchの説明追加
- [ ] CHANGELOG.md に破壊的変更を記載
  - [ ] LifecycleEventの`watcher`フィールド追加
  - [ ] 移行ガイド（便利メソッド使用）
  - [ ] 新機能の説明
- [ ] API ドキュメント（rustdoc）の充実
  - [ ] ActorContext::watch/unwatchの詳細説明
  - [ ] Actor::on_terminatedの使用例
  - [ ] LifecycleEvent::watcherの説明

#### 移行ガイド作成
- [ ] Akka/Pekkoからの移行ガイド作成
  - [ ] コード比較例
  - [ ] ベストプラクティス
  - [ ] よくある質問（FAQ）

#### 検証
- [ ] exampleが正常に実行できる
- [ ] ドキュメントが正確で分かりやすい
- [ ] 移行ガイドが実用的

### Phase 7: 最終検証

#### OpenSpec検証
- [ ] `openspec validate --strict add-actor-death-watch` が成功

#### 品質チェック
- [ ] 全テストが成功
- [ ] カバレッジ目標達成
- [ ] ドキュメント完成
- [ ] サンプルコード動作確認
- [ ] パフォーマンス要件達成

#### リリース準備
- [ ] CHANGELOG.md 最終確認
- [ ] バージョン番号決定（破壊的変更のためメジャーバージョンアップ）
- [ ] リリースノート作成

## 実装方針メモ

### 破壊的変更の対処
- LifecycleEvent構造体のフィールド追加は破壊的変更
- 便利メソッド（`new_started`等）を提供することで影響を最小化
- パターンマッチングでは`..`を使うことを推奨

### no_std対応
- 全ての実装はRuntimeToolbox抽象化を使用
- actor-coreで実装し、actor-stdは自動的に利用可能

### テスト戦略
- 単体テスト: 各コンポーネントを個別にテスト
- 統合テスト: death_watch.rsで全体的な動作を確認
- エッジケース: 循環監視、重複watch、既に停止したアクターのwatch等
