# 設計案: Create/Recreate SystemMessage 化

## 方針概要
- actor 起動・再起動を `SystemMessage` の variant として扱い、mailbox を経由して ActorCell に届ける。
- 既存の `ActorCell::create()` や `restart()` が行っている `pre_start`／`post_stop` 呼び出しは `handle_create` / `handle_recreate` に集約する。
- spawn / supervisor フローは ActorCell の public API を直接呼ぶのではなく、システムメッセージを enqueue するだけに留める。

## 主要コンポーネント

### 1. SystemMessage::Create / Recreate
- `SystemMessage` enum に2 variant を追加。
- 追加テストで `AnyMessageGeneric` への変換や downcast を検証。

### 2. ActorCell
- `invoke_system_message` に新分岐を追加。
- `handle_create`：`run_pre_start(LifecycleStage::Started)` を呼び出し、結果を返すだけの薄いヘルパー。
- `handle_recreate`：既存 `restart()` のロジックを移植。`post_stop` → actor 再生成 → `run_pre_start(LifecycleStage::Restarted)`。
- system queue が user queue より常に先に処理される前提を確認する統合テストを追加し、Create 完了前に通常メッセージが実行されないことを保証する。
- spawn 時に直接 `pre_start` を呼んでいた箇所は、`SystemMessage::Create` を mailbox に enqueue して dispatcher に処理させる。

### 3. SystemState / ActorSystem
- `ActorSystem::spawn_with_parent`
  - ActorCell を生成したら `SystemMessage::Create` を送る。
  - 送信エラー時は spawn を失敗にして rollback。
  - enqueue の完了および Create 実行結果は oneshot channel で待ち合わせ、`pre_start` 成功まで `spawn_with_parent` は成功を返さない。
- `SystemState::handle_failure`
  - Restart 指示の場合、対象 PID に `SystemMessage::Recreate` を送信。
  - Stop 指示はこれまで通り `SystemMessage::Stop`。
  - `SystemMessage::Recreate` の enqueue に失敗した場合は `SystemMessage::Stop` へフォールバックし、Supervisor に Escalate する。

## 移行ステップ
1. SystemMessage variant 追加 → ActorCell 新 handler 実装。
2. spawn/restart 呼び出しを順に SystemMessage 経路へ切り替え。
3. SupervisorStrategy のテストを更新して `post_stop` → 再生成 → `pre_start` (Restart) の呼び出しタイミングを検証。

## エラーハンドリング

- **Create送信失敗**: mailbox が閉じている等で `SystemMessage::Create` を送れない場合は即座に `rollback_spawn` を実行し、`SpawnError` を返す。
- **Create完了待ち**: `spawn_with_parent` は dispatcher からの ACK を oneshot で受け取り、`pre_start` 成否を呼び出し元へ伝播する。ACK がエラーなら rollback する。
- **Recreate送信失敗**: supervisor が Restart を指示しても送信できない場合は対象 PID に `SystemMessage::Stop` を enqueue し、Escalate 経路に通知して停止させる。
- **Lifecycle hook 失敗**: `pre_start` / `post_stop` の失敗は SystemMessage 処理結果として dispatcher に返し、既存の supervisor 戦略で再処理される。

## EventStream統合

- `handle_create` が `LifecycleStage::Started` を publish し、`handle_recreate` は `LifecycleStage::Stopped` → `LifecycleStage::Restarted` を順に publish する。
- SystemMessage 化により発火タイミングは mailbox に集約されるが、EventStream API/購読者の体験は変えない。
- Create/Recreate エラー時には Stage イベントを重複発火させないよう、publish を `run_pre_start` 前後の単一地点に限定する。

## 検討事項
- Failure の SystemMessage 化は別変更で扱う（SupervisorState との調停が必要）。
- dispatcher ACK の実装方式（oneshot / inbox future）は実装段階で最適なものを選ぶ。
- ログや LifecycleEvent の重複発火がないよう、`run_pre_start`/`handle_stop` の呼び出し経路を一箇所に制限する。
- EventStream の既存実装は変更しない。SystemMessage 経由で制御を統一しつつ、Publish/Subscribe（ライフサイクルの監視や外部ツール連携）用途では引き続き EventStream が利用できる必要があるため、削除・機能縮退は行わない。
