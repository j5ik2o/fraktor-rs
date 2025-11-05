# pub(crate)に変更すべきメソッド一覧

このドキュメントは、actor-core内で定義されpublicになっているが、実際にはactor-core内部でのみ使用されているメソッドをリストアップします。
これらは`pub(crate)`に変更することで、APIの表面積を減らし、内部実装の詳細を隠蔽すべき候補です。

## エグゼクティブサマリー

### API表面積削減効果

- **現在の公開メソッド総数**: 48個（SystemState: 27, Dispatcher: 9, Mailbox: 12）
- **内部実装として隠蔽可能**: 36個
- **API表面積削減率**: 約**75%**

### 主要コンポーネント別の削減効果

| コンポーネント | 公開メソッド数 | pub(crate)推奨 | 削減率 |
|--------------|--------------|--------------|--------|
| SystemStateGeneric | 27個 | 18個 | 66% |
| DispatcherGeneric | 9個 | 7個 | 77% |
| MailboxGeneric | 12個 | 11個 | 91% |

### 期待される効果

1. **セキュリティ向上**: 内部実装の詳細が外部から隠蔽される
2. **保守性向上**: 内部実装を自由に変更できる
3. **ドキュメント改善**: 公開APIが明確になり、ユーザーが理解しやすい
4. **コンパイル時間短縮**: 公開APIの変更が少なくなり、再コンパイルが減る

## 分析基準

- actor-std では使用されていない（actor-stdはラッパーとして機能）
- actor-core の内部実装でのみ使用されている
- テストコードでのみ使用されている場合も含む

## SystemStateGeneric の内部専用メソッド候補

### 🔒 pub(crate)に変更推奨

| メソッド | 使用箇所 | 理由 |
|---------|----------|------|
| `register_cell` | system/base.rs | ActorSystemGenericの内部処理でのみ使用 |
| `remove_cell` | actor_prim/actor_cell.rs, system/base.rs | アクタセルのライフサイクル管理の内部処理 |
| `cell` | system/base.rs, system/system_state.rs | 内部状態へのアクセス、公開APIでは不要 |
| `assign_name` | system/base.rs | spawn処理の内部実装 |
| `release_name` | actor_prim/actor_cell.rs, system/base.rs | アクタ終了時の内部処理 |
| `set_user_guardian` | system/base.rs | システム初期化の内部処理 |
| `clear_guardian` | system/base.rs | ガーディアン管理の内部処理 |
| `user_guardian` | system/base.rs | 内部状態へのアクセス |
| `register_child` | system/base.rs | 親子関係管理の内部処理 |
| `unregister_child` | actor_prim/actor_cell.rs | アクタ終了時の内部処理 |
| `child_pids` | system/base.rs | 内部状態へのアクセス（公開APIはActorSystem経由） |
| `send_system_message` | system/system_state.rs内部 | システムメッセージ送信の内部実装 |
| `record_send_error` | actor_prim/actor_ref | エラーハンドリングの内部処理 |
| `notify_failure` | actor_prim/actor_cell.rs | 障害通知の内部処理 |
| `register_ask_future` | actor_prim/actor_ref | askパターンの内部実装 |
| `mark_terminated` | system/base.rs | 終了処理の内部実装 |
| `termination_future` | system/base.rs | 終了待機の内部実装 |
| `drain_ready_ask_futures` | system/base.rs | askフューチャーのポーリング内部処理 |

### 📖 publicのまま維持すべき

| メソッド | 理由 |
|---------|------|
| `new` | コンストラクタとして外部から使用される可能性 |
| `allocate_pid` | テストヘルパーとして有用 |
| `event_stream` | イベントストリームへの公開アクセス |
| `dead_letters` | デッドレター情報の公開アクセス |
| `publish_event` | イベント発行の公開API |
| `emit_log` | ログ出力の公開API |
| `is_terminated` | システム状態の確認 |
| `monotonic_now` | 時刻取得のユーティリティ |
| `user_guardian_pid` | ガーディアンPIDの取得 |

## DispatcherGeneric の内部専用メソッド候補

### 🔒 pub(crate)に変更推奨

| メソッド | 使用箇所 | 理由 |
|---------|----------|------|
| `register_invoker` | actor_prim/actor_cell.rs | アクタセル初期化の内部処理 |
| `enqueue_user` | dispatcher_sender.rs | ディスパッチャー送信者の内部実装 |
| `enqueue_system` | dispatcher_sender.rs | システムメッセージの内部処理 |
| `schedule` | dispatcher_sender.rs | スケジューリングの内部実装 |
| `mailbox` | actor_prim/actor_cell.rs, props/base.rs | メールボックスアクセスの内部実装 |
| `create_waker` | dispatcher_sender.rs | Waker生成の内部実装 |
| `into_sender` | actor_prim/actor_cell.rs | 送信者への変換内部処理 |

### 📖 publicのまま維持すべき

| メソッド | 理由 |
|---------|------|
| `new` | コンストラクタ |
| `with_inline_executor` | テスト用コンストラクタ |

## MailboxGeneric の内部専用メソッド候補

### 🔒 pub(crate)に変更推奨

| メソッド | 使用箇所 | 理由 |
|---------|----------|------|
| `set_instrumentation` | actor_prim/actor_cell.rs | インストルメンテーション設定の内部処理 |
| `enqueue_system` | dispatcher/base.rs | システムメッセージキューイングの内部実装 |
| `enqueue_user` | dispatcher/base.rs | ユーザーメッセージキューイングの内部実装 |
| `enqueue_user_future` | dispatcher/base.rs | 非同期メッセージキューイングの内部実装 |
| `poll_user_future` | dispatcher/base.rs | 非同期メッセージポーリングの内部実装 |
| `dequeue` | dispatcher/dispatcher_core.rs | メッセージデキューの内部実装 |
| `suspend` | actor_prim/actor_cell.rs | メールボックス一時停止の内部処理 |
| `resume` | actor_prim/actor_cell.rs | メールボックス再開の内部処理 |
| `is_suspended` | 未使用？ | 状態確認、テスト用か |
| `user_len` | 未使用？ | 長さ取得、テスト用か |
| `system_len` | 未使用？ | 長さ取得、テスト用か |

### 📖 publicのまま維持すべき

| メソッド | 理由 |
|---------|------|
| `new` | コンストラクタ |

## ActorCellGeneric の内部専用メソッド候補

ActorCellGeneric自体が内部実装の詳細なので、全メソッドを`pub(crate)`にすべき。

## その他の型

### EventStreamGeneric

現在の公開メソッドは妥当。`subscribe_arc`、`unsubscribe`、`publish`は公開APIとして適切。

### DeadLetterGeneric

- `record_send_error`、`record_entry` → `pub(crate)` （内部記録処理）
- `entries` → public（公開API）
- `new`、`with_default_capacity` → public（コンストラクタ）

### PropsGeneric

現在の公開メソッドは妥当。全て公開APIとして適切。

### ActorRefGeneric

現在の公開メソッドは妥当。`new`、`tell`、`ask`、`null`は公開APIとして適切。
ただし`with_system`は内部実装の可能性あり（要確認）。

## 実装方針

1. **Phase 1**: 明らかに内部実装のメソッド
   - SystemStateGenericの大部分
   - DispatcherGenericの大部分
   - MailboxGenericのキューイング/デキュー関連

2. **Phase 2**: テスト用途と思われるメソッド
   - `is_suspended`、`user_len`、`system_len`など
   - これらは`#[cfg(test)]`付きで公開するか検討

3. **Phase 3**: 境界ケース
   - ActorRefGeneric::with_system
   - その他、判断が難しいメソッド

## クイックリファレンス：優先度別変更リスト

### 🔴 高優先度（Phase 1）: 明らかに内部実装

```rust
// SystemStateGeneric
pub(crate) fn register_cell(...)
pub(crate) fn remove_cell(...)
pub(crate) fn cell(...)
pub(crate) fn send_system_message(...)
pub(crate) fn notify_failure(...)
pub(crate) fn mark_terminated(...)

// DispatcherGeneric
pub(crate) fn register_invoker(...)
pub(crate) fn enqueue_user(...)
pub(crate) fn enqueue_system(...)
pub(crate) fn schedule(...)
pub(crate) fn create_waker(...)
pub(crate) fn into_sender(...)

// MailboxGeneric
pub(crate) fn enqueue_system(...)
pub(crate) fn enqueue_user(...)
pub(crate) fn enqueue_user_future(...)
pub(crate) fn poll_user_future(...)
pub(crate) fn dequeue(...)
pub(crate) fn suspend(...)
pub(crate) fn resume(...)
```

### 🟡 中優先度（Phase 2）: 名前/子管理

```rust
// SystemStateGeneric
pub(crate) fn assign_name(...)
pub(crate) fn release_name(...)
pub(crate) fn set_user_guardian(...)
pub(crate) fn clear_guardian(...)
pub(crate) fn user_guardian(...)
pub(crate) fn register_child(...)
pub(crate) fn unregister_child(...)
pub(crate) fn child_pids(...)
```

### 🟢 低優先度（Phase 3）: テスト用/Future管理

```rust
// SystemStateGeneric
pub(crate) fn register_ask_future(...)
pub(crate) fn drain_ready_ask_futures(...)
pub(crate) fn record_send_error(...)
pub(crate) fn termination_future(...)

// MailboxGeneric
pub(crate) fn set_instrumentation(...)
pub(crate) fn is_suspended(...)
pub(crate) fn user_len(...)
pub(crate) fn system_len(...)

// DispatcherGeneric
pub(crate) fn mailbox(...)
```

## 実装チェックリスト

- [ ] Phase 1: 高優先度メソッド（21個）の変更
  - [ ] SystemStateGeneric (6個)
  - [ ] DispatcherGeneric (7個)
  - [ ] MailboxGeneric (7個)
  - [ ] テスト実行・確認

- [ ] Phase 2: 中優先度メソッド（8個）の変更
  - [ ] SystemStateGeneric (8個)
  - [ ] テスト実行・確認

- [ ] Phase 3: 低優先度メソッド（7個）の変更
  - [ ] SystemStateGeneric (4個)
  - [ ] MailboxGeneric (4個)
  - [ ] DispatcherGeneric (1個)
  - [ ] テスト実行・確認

- [ ] 最終確認
  - [ ] 全テストパス
  - [ ] cargo doc でドキュメント生成確認
  - [ ] examples がビルド・実行可能
  - [ ] CI/CD パス

## 次のステップ

1. このリストをレビュー
2. 各メソッドについて本当に公開が不要か確認
3. `pub(crate)`への変更を段階的に適用（Phase 1 → Phase 2 → Phase 3）
4. 各フェーズでテストが通ることを確認
5. 必要に応じて追加の公開メソッドを検討

## 注意事項

- `pub(crate)`に変更後も、actor-std経由では同じ機能が利用可能
- テストコードから直接アクセスしている箇所は、テストヘルパーを追加する必要がある場合あり
- breaking changeとなるため、メジャーバージョンアップ時に実施推奨
