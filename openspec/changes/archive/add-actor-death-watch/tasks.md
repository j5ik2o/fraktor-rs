## 実装タスクリスト

### Phase 1: コアインフラ構築

#### SystemMessage拡張
- [x] `modules/actor-core/src/messaging/system_message.rs` - Watch/Unwatch/Terminated variant追加
  - [x] `Watch(Pid)` variant追加
  - [x] `Unwatch(Pid)` variant追加
  - [x] `Terminated(Pid)` variant追加
  - [x] Debugトレイト自動導出の確認
  - [x] 単体テスト追加

#### 検証
- [x] `cargo test --package cellactor-core` が全てパス
- [x] SystemMessage新variantが正しく動作することを確認

### Phase 2: ActorCell拡張

#### ActorCellフィールド追加
- [x] `modules/actor-core/src/actor_prim/actor_cell.rs` - watchersフィールドとメソッド追加
  - [x] `watchers: ToolboxMutex<Vec<Pid>, TB>` フィールド追加
  - [x] `new()` メソッドでwatchersを初期化
  - [x] `handle_watch(watcher: Pid)` メソッド実装（冪等性保証）
  - [x] `handle_unwatch(watcher: Pid)` メソッド実装
  - [x] `notify_watchers_on_stop()` メソッド実装
    - [x] 各監視者向けに`SystemMessage::Terminated(self.pid)`送信
    - [x] システムワイド用の`LifecycleEvent::new_stopped()`は従来通り発行
  - [x] `handle_terminated(terminated_pid: Pid)` メソッド実装
    - [x] `&mut self.actor`をロック
    - [x] `ActorContext::new()`で`&mut ActorContext`生成
    - [x] `actor.on_terminated(&mut ctx, terminated_pid)`呼び出し
  - [x] `stop()` メソッドに`notify_watchers_on_stop()`呼び出し追加
  - [x] `stop()` メソッドでwatchersリストをクリア

#### 単体テスト
- [x] watchersリストの追加・削除テスト
- [x] 冪等性テスト（同じPidを複数回watch）
- [x] 停止時のSystemMessage::Terminated送信テスト
- [x] watchersリストのクリアテスト
- [x] handle_terminatedのActorContext生成テスト

#### 検証
- [x] `cargo test --package cellactor-core` が全てパス
- [x] ActorCellの単体テストが全て成功

### Phase 3: API追加

#### ActorContext拡張
- [x] `modules/actor-core/src/actor_prim/actor_context.rs` - watch/unwatch API追加
  - [x] `watch(target: &ActorRefGeneric<TB>)` メソッド実装
    - [x] SystemMessage::Watch(self_pid)を送信
    - [x] エラーハンドリング
    - [x] docコメント追加
  - [x] `unwatch(target: &ActorRefGeneric<TB>)` メソッド実装
    - [x] SystemMessage::Unwatch(self_pid)を送信
    - [x] エラーハンドリング
    - [x] docコメント追加
  - [x] `spawn_child_watched(props: &PropsGeneric<TB>)` 便利メソッド実装
    - [x] spawn_childを呼び出し
    - [x] 自動的にwatchを呼び出し
    - [x] エラーハンドリング
    - [x] docコメント追加

#### Actorトレイト拡張
- [x] `modules/actor-core/src/actor_prim/actor.rs` - on_terminated追加
  - [x] `on_terminated(ctx: &mut ActorContext, terminated: Pid)` デフォルト実装追加
  - [x] デフォルト実装は`Ok(())`を返す
  - [x] docコメント追加（ActorCell::handle_terminatedから呼ばれることを明記）
  - [x] 使用例を含むドキュメント

#### 検証
- [x] `cargo test --package cellactor-core` が全てパス
- [x] APIドキュメントが正しく生成される

### Phase 4: SystemState統合

#### SystemMessage処理追加
- [x] `modules/actor-core/src/system/system_state.rs` - Watch/Unwatch/Terminated処理追加
  - [x] `process_system_message()` にWatch処理を追加
    - [x] ActorCellの`handle_watch()`呼び出し
    - [x] 対象アクターが既に停止している場合、即座にTerminatedを送信
    - [x] エラーハンドリング
  - [x] `process_system_message()` にUnwatch処理を追加
    - [x] ActorCellの`handle_unwatch()`呼び出し
    - [x] エラーハンドリング
  - [x] `process_system_message()` にTerminated処理を追加
    - [x] ActorCellの`handle_terminated()`呼び出し
    - [x] エラーハンドリング

#### 検証
- [x] `cargo test --package cellactor-core` が全てパス
- [x] SystemMessageの処理が正常に動作

### Phase 5: テストと検証

#### 統合テスト作成
- [x] `modules/actor-core/tests/death_watch.rs` 新規作成
  - [x] 基本的な監視テスト（watch → 子停止 → on_terminated呼び出し）
  - [x] unwatch後は通知されないテスト
  - [x] 複数監視者が全員通知を受け取るテスト
  - [x] システムワイドLifecycleEvent(Stopped)も発行されるテスト
  - [x] 循環監視でもデッドロックしないテスト
  - [x] 既に停止したアクターをwatchすると即座にTerminatedが送られるテスト
  - [x] 同じアクターを複数回watchしても冪等テスト
  - [x] `spawn_child_watched`の動作テスト
  - [x] watch直後に停止してもTerminatedを受け取れるテスト（レース条件）

#### no_std環境テスト
- [x] actor-coreのno_std環境でのビルド確認
- [x] 全機能がno_std環境で動作することを確認

#### actor-std環境テスト
- [x] actor-stdで同じAPIが利用可能であることを確認
- [x] actor-stdのテストが全てパス
- [x] `cargo test --package cellactor-std` が全てパス

#### 全体テスト
- [x] `cargo test --workspace` が全てパス
- [x] `cargo clippy --workspace` が警告なし
- [x] `cargo fmt --check` がパス
- [x] カバレッジ ≥ 90% を確認

#### 検証
- [x] 全ての成功基準を満たす
- [x] パフォーマンステスト（メモリ使用量、メッセージ送信数）
- [x] エッジケース処理の確認

### Phase 6: ドキュメントと例

#### サンプルコード作成
- [x] `modules/actor-std/examples/death_watch.rs` 新規作成
  - [x] 基本的なwatch/unwatchの使用例
  - [x] 子アクターの再起動パターン
  - [x] `spawn_child_watched`の使用例
  - [x] 複数の子アクターを監視する例

#### ドキュメント更新
- [x] `README.md` にwatch/unwatchの説明追加
- [x] CHANGELOG.md に新機能を記載
  - [x] SystemMessage::Watch/Unwatch/Terminated追加
  - [x] ActorContext::watch/unwatch/spawn_child_watched追加
  - [x] Actor::on_terminated追加
  - [x] ActorCell::watchers/handle_watch/handle_unwatch/handle_terminated追加
  - [x] NON-BREAKING（既存APIに変更なし）
- [x] API ドキュメント（rustdoc）の充実
  - [x] ActorContext::watch/unwatchの詳細説明
  - [x] Actor::on_terminatedの使用例
  - [x] ActorCell::handle_terminatedの呼び出しフロー

#### 移行ガイド作成
- [x] Akka/Pekkoからの移行ガイド作成
  - [x] コード比較例
  - [x] ベストプラクティス
  - [x] よくある質問（FAQ）

#### 検証
- [x] exampleが正常に実行できる
- [x] ドキュメントが正確で分かりやすい
- [x] 移行ガイドが実用的

### Phase 7: 最終検証

#### OpenSpec検証
- [x] `openspec validate --strict add-actor-death-watch` が成功

#### 品質チェック
- [x] 全テストが成功
- [x] カバレッジ目標達成
- [x] ドキュメント完成
- [x] サンプルコード動作確認
- [x] パフォーマンス要件達成

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
