# 提案: actor-coreの公開API表面積の削減

## Why

actor-core内で定義されている公開メソッドのうち、実際には内部実装の詳細として`pub(crate)`に変更すべきメソッドが大量に存在します（全体の約75%）。

### 現状の問題点

1. **過剰な公開API**: 48個の公開メソッドのうち36個（75%）がactor-core内部でのみ使用
2. **曖昧なAPI境界**: どのメソッドがユーザー向けか内部実装かが不明確
3. **保守性の低下**: 内部実装の変更が外部に影響を及ぼすリスク
4. **ドキュメントの肥大化**: 内部実装まで公開ドキュメントに含まれる

### 期待される効果

1. **セキュリティ向上**: 内部実装の詳細が外部から隠蔽される
2. **保守性向上**: 内部実装を自由に変更できるようになる
3. **ドキュメント改善**: 公開APIが明確になり、ユーザーが理解しやすくなる
4. **コンパイル時間短縮**: 公開APIの変更が少なくなり、再コンパイルが減る

## What Changes

### 影響を受けるコンポーネント

| コンポーネント | 現在の公開メソッド | pub(crate)化 | 削減率 |
|--------------|-----------------|-------------|--------|
| SystemStateGeneric | 27個 | 18個 | 66% |
| DispatcherGeneric | 9個 | 7個 | 77% |
| MailboxGeneric | 12個 | 11個 | 91% |
| **合計** | **48個** | **36個** | **75%** |

### 段階的実装

#### Phase 1: 高優先度（明らかな内部実装） - 21個のメソッド

**SystemStateGeneric (6個)**:
- `register_cell` - アクタセル登録の内部処理
- `remove_cell` - アクタセル削除の内部処理
- `cell` - 内部状態へのアクセス
- `send_system_message` - システムメッセージ送信の内部実装
- `notify_failure` - 障害通知の内部処理
- `mark_terminated` - 終了処理の内部実装

**DispatcherGeneric (7個)**:
- `register_invoker` - メッセージ呼び出し元の登録
- `enqueue_user` - ユーザーメッセージのキューイング
- `enqueue_system` - システムメッセージのキューイング
- `schedule` - ディスパッチャーのスケジューリング
- `create_waker` - Waker生成
- `into_sender` - 送信者への変換
- `mailbox` - メールボックスへのアクセス

**MailboxGeneric (7個)**:
- `enqueue_system` - システムメッセージキューイング
- `enqueue_user` - ユーザーメッセージキューイング
- `enqueue_user_future` - 非同期メッセージキューイング
- `poll_user_future` - 非同期メッセージポーリング
- `dequeue` - メッセージデキュー
- `suspend` - メールボックス一時停止
- `resume` - メールボックス再開

#### Phase 2: 中優先度（名前管理・子管理） - 8個のメソッド

**SystemStateGeneric (8個)**:
- `assign_name` - アクタ名の割り当て
- `release_name` - アクタ名の解放
- `set_user_guardian` - ユーザーガーディアンの設定
- `clear_guardian` - ガーディアンのクリア
- `user_guardian` - ユーザーガーディアンの取得
- `register_child` - 子アクタの登録
- `unregister_child` - 子アクタの登録解除
- `child_pids` - 子アクタPIDの取得

#### Phase 3: 低優先度（Future管理・テスト用） - 7個のメソッド

**SystemStateGeneric (4個)**:
- `register_ask_future` - askフューチャーの登録
- `drain_ready_ask_futures` - 準備完了フューチャーの取得
- `record_send_error` - 送信エラーの記録
- `termination_future` - 終了フューチャーの取得

**MailboxGeneric (3個)**:
- `set_instrumentation` - インストルメンテーション設定
- `is_suspended` - 一時停止状態の確認
- `user_len` - ユーザーメッセージキューの長さ
- `system_len` - システムメッセージキューの長さ

### 公開のまま維持するメソッド（12個）

actor-stdやexamplesから使用されている、または公開APIとして必要なメソッド:

- SystemStateGeneric: `new`, `allocate_pid`, `event_stream`, `dead_letters`, `publish_event`, `emit_log`, `is_terminated`, `monotonic_now`, `user_guardian_pid`
- DispatcherGeneric: `new`, `with_inline_executor`
- MailboxGeneric: `new`

## Impact

### 破壊的変更

**BREAKING**: セマンティックバージョニングにおけるメジャーバージョンアップが必要

### 影響範囲

- **Affected specs**: api-visibility
- **Affected code**:
  - `modules/actor-core/src/system/system_state.rs`
  - `modules/actor-core/src/dispatcher/base.rs`
  - `modules/actor-core/src/mailbox/base.rs`

### コンポーネント別影響

1. **actor-std**: 影響なし（ラッパー経由で同じ機能を提供）
2. **examples**: 影響なし（公開APIのみ使用）
3. **直接actor-coreを使用するユーザー**: 破壊的変更
   - 内部実装メソッドに直接アクセスしている場合、コンパイルエラー
   - actor-std経由での使用を推奨

### 移行ガイド

内部実装メソッドに直接アクセスしているコードは、以下のいずれかの対応が必要:

1. **actor-std経由で使用**: 推奨アプローチ
2. **公開APIで代替**: 同等の機能が公開APIで提供されている場合
3. **機能要求**: 必要な機能が公開されていない場合は issue で要求

### リスクと緩和策

**リスク**:
1. テストコードからの直接アクセス: テストが`pub(crate)`メソッドに依存している可能性
2. 隠れた外部依存: 予期しない外部クレートからの使用

**緩和策**:
1. 段階的実装: Phase 1→2→3で影響を確認しながら進める
2. テストヘルパー追加: 必要に応じて`#[cfg(test)] pub`を使用
3. ドキュメント更新: CHANGELOG.mdとMIGRATION.mdを更新

### 成功基準

1. ✅ 全36個のメソッドが`pub(crate)`化される
2. ✅ 全テストがパスする
3. ✅ `./scripts/ci-check.sh all`が成功する
4. ✅ examplesが正常に動作する
5. ✅ APIドキュメントが明確化される（`cargo doc`で内部メソッドが非表示）

### 関連資料

- 詳細分析: `claudedocs/pub_crate_candidates.md`
- 現在のAPI: `modules/actor-core/src/system/system_state.rs`
- ラッパー実装: `modules/actor-std/src/system/base.rs`
