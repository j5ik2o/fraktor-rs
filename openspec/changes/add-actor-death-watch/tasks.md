## 実装タスクリスト

### Phase 1: コアインフラ構築

#### SystemMessage拡張
- [ ] `modules/actor-core/src/messaging/system_message.rs` - Watch/Unwatch/Terminated variant追加
  - [ ] `Watch(Pid)` variant追加
  - [ ] `Unwatch(Pid)` variant追加
  - [ ] `Terminated(Pid)` variant追加
  - [ ] Debugトレイト自動導出の確認
  - [ ] 単体テスト追加

#### 検証
- [ ] `cargo test --package cellactor-core` が全てパス
- [ ] SystemMessage新variantが正しく動作することを確認

### Phase 2: ActorCell拡張

#### ActorCellフィールド追加
- [ ] `modules/actor-core/src/actor_prim/actor_cell.rs` - watchersフィールドとメソッド追加
  - [ ] `watchers: ToolboxMutex<Vec<Pid>, TB>` フィールド追加
  - [ ] `new()` メソッドでwatchersを初期化
  - [ ] `handle_watch(watcher: Pid)` メソッド実装（冪等性保証）
  - [ ] `handle_unwatch(watcher: Pid)` メソッド実装
  - [ ] `notify_watchers_on_stop()` メソッド実装
    - [ ] 各監視者向けに`SystemMessage::Terminated(self.pid)`送信
    - [ ] システムワイド用の`LifecycleEvent::new_stopped()`は従来通り発行
  - [ ] `handle_terminated(terminated_pid: Pid)` メソッド実装
    - [ ] `&mut self.actor`をロック
    - [ ] `ActorContext::new()`で`&mut ActorContext`生成
    - [ ] `actor.on_terminated(&mut ctx, terminated_pid)`呼び出し
  - [ ] `stop()` メソッドに`notify_watchers_on_stop()`呼び出し追加
  - [ ] `stop()` メソッドでwatchersリストをクリア

#### 単体テスト
- [ ] watchersリストの追加・削除テスト
- [ ] 冪等性テスト（同じPidを複数回watch）
- [ ] 停止時のSystemMessage::Terminated送信テスト
- [ ] watchersリストのクリアテスト
- [ ] handle_terminatedのActorContext生成テスト

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
  - [ ] docコメント追加（ActorCell::handle_terminatedから呼ばれることを明記）
  - [ ] 使用例を含むドキュメント

#### 検証
- [ ] `cargo test --package cellactor-core` が全てパス
- [ ] APIドキュメントが正しく生成される

### Phase 4: SystemState統合

#### SystemMessage処理追加
- [ ] `modules/actor-core/src/system/system_state.rs` - Watch/Unwatch/Terminated処理追加
  - [ ] `process_system_message()` にWatch処理を追加
    - [ ] ActorCellの`handle_watch()`呼び出し
    - [ ] 対象アクターが既に停止している場合、即座にTerminatedを送信
    - [ ] エラーハンドリング
  - [ ] `process_system_message()` にUnwatch処理を追加
    - [ ] ActorCellの`handle_unwatch()`呼び出し
    - [ ] エラーハンドリング
  - [ ] `process_system_message()` にTerminated処理を追加
    - [ ] ActorCellの`handle_terminated()`呼び出し
    - [ ] エラーハンドリング

#### 検証
- [ ] `cargo test --package cellactor-core` が全てパス
- [ ] SystemMessageの処理が正常に動作

### Phase 5: テストと検証

#### 統合テスト作成
- [ ] `modules/actor-core/tests/death_watch.rs` 新規作成
  - [ ] 基本的な監視テスト（watch → 子停止 → on_terminated呼び出し）
  - [ ] unwatch後は通知されないテスト
  - [ ] 複数監視者が全員通知を受け取るテスト
  - [ ] システムワイドLifecycleEvent(Stopped)も発行されるテスト
  - [ ] 循環監視でもデッドロックしないテスト
  - [ ] 既に停止したアクターをwatchすると即座にTerminatedが送られるテスト
  - [ ] 同じアクターを複数回watchしても冪等テスト
  - [ ] `spawn_child_watched`の動作テスト
  - [ ] watch直後に停止してもTerminatedを受け取れるテスト（レース条件）

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
- [ ] パフォーマンステスト（メモリ使用量、メッセージ送信数）
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
- [ ] CHANGELOG.md に新機能を記載
  - [ ] SystemMessage::Watch/Unwatch/Terminated追加
  - [ ] ActorContext::watch/unwatch/spawn_child_watched追加
  - [ ] Actor::on_terminated追加
  - [ ] ActorCell::watchers/handle_watch/handle_unwatch/handle_terminated追加
  - [ ] NON-BREAKING（既存APIに変更なし）
- [ ] API ドキュメント（rustdoc）の充実
  - [ ] ActorContext::watch/unwatchの詳細説明
  - [ ] Actor::on_terminatedの使用例
  - [ ] ActorCell::handle_terminatedの呼び出しフロー

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

## 実装方針メモ

### SystemMessage::Terminatedアプローチの利点
- EventStreamを経由せず、監視者のメールボックスに直接配送
- O(n)の効率（n=監視者数、EventStream方式のO(n×m)と比較してm倍効率的）
- システムメッセージとして優先処理されるため、順序保証が得られる
- `ActorCell::handle_terminated()`が`&mut ActorContext`を生成するため、`on_terminated`で完全なアクター操作が可能

### EventStream方式の問題点（回避済み）
- `EventStreamSubscriber::on_event(&self, ...)`は`&self`のみで`&mut ActorContext`が取得できない
- EventStreamはアクター処理スレッド外で動作するため、`&mut self`への安全なアクセスができない
- 全サブスクライバーにブロードキャストされるため、O(n×m)の非効率（n=監視者数、m=全サブスクライバー数）

### no_std対応
- 全ての実装はRuntimeToolbox抽象化を使用
- actor-coreで実装し、actor-stdは自動的に利用可能

### テスト戦略
- 単体テスト: 各コンポーネントを個別にテスト
- 統合テスト: death_watch.rsで全体的な動作を確認
- エッジケース: 循環監視、重複watch、既に停止したアクターのwatch、レース条件等
