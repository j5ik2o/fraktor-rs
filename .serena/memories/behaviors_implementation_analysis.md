# Behaviors実装調査結果

## 1. behaviors.rs の実装状況
**ファイル**: `modules/actor-core/src/typed/behaviors.rs`

Pekko-inspiredなヘルパー構造体で、Behavior型を構築するためのファクトリーメソッドを提供する。

### 実装されているメソッド:
- `same<M, TB>()` - 現在のBehaviorを保持する指令を返す
- `stopped<M, TB>()` - アクターの停止を指示する指令を返す
- `ignore<M, TB>()` - 受信メッセージをサイレントに破棄するBehavior
- `setup<M, TB, F>(factory: F)` - アクター起動時にコンテキストアクセスしてBehavior構築
- `receive_message<M, TB, F>(handler: F)` - メッセージハンドラーベースのBehavior
- `receive_signal<M, TB, F>(handler: F)` - シグナルハンドラーベースのBehavior

## 2. Behavior型の実装
**ファイル**: `modules/actor-core/src/typed/behavior.rs`

### 構造体定義:
```rust
pub struct Behavior<M, TB = NoStdToolbox> {
  directive:       BehaviorDirective,
  message_handler: Option<MessageHandler<M, TB>>,
  signal_handler:  Option<SignalHandler<M, TB>>,
}
```

メッセージとシグナルハンドラーを任意にアタッチ可能。

## 3. BehaviorDirective種類
4つのディレクティブが定義されている:

1. **Same** - ランタイムが前のBehavior インスタンスを再利用する
2. **Stopped** - アクターが優雅に停止する
3. **Ignore** - Behaviorは活動中だがメッセージをサイレントにドロップ
4. **Active** - ハンドラーを新たなBehaviorとして使用

## 4. BehaviorSignal種類
**ファイル**: `modules/actor-core/src/typed/behavior_signal.rs`

3つのシグナルが定義:
- **Started** - アクター起動完了
- **Stopped** - アクター停止前
- **Terminated(Pid)** - ウォッチ対象アクターが終了した

## 5. BehaviorRunner/execute フロー
**ファイル**: `modules/actor-core/src/typed/behavior_runner.rs`

TypedActorライフサイクルを管理:
- `pre_start()` → BehaviorSignal::Started を dispatch
- `receive()` → メッセージハンドリング
- `post_stop()` → BehaviorSignal::Stopped を dispatch
- `on_terminated()` → BehaviorSignal::Terminated を dispatch

transition適用ロジック:
- Same/Ignore → 状態変更なし
- Stopped → self.stopping=true、アクター停止
- Active → 新しいBehaviorで置き換え

## 6. DeadLetter実装
**ファイル**: `modules/actor-core/src/dead_letter/`

### DeadLetterReason enum:
- **MailboxFull** - メールボックス容量超過
- **MailboxSuspended** - メールボックス一時停止中
- **RecipientUnavailable** - 受信者不在/クローズ
- **MissingRecipient** - reply対象なし
- **FatalActorError** - アクター実行エラー
- **ExplicitRouting** - システムロジックによる明示的ルーティング

### DeadLetterEntry:
- message: 未配信メッセージ
- reason: DeadLetterReason
- recipient: Option<Pid>
- timestamp: Duration

DeadLetterGenericはEventStreamへパブリッシュする。

## 7. unhandledメッセージ処理
現在の実装では:
- メッセージハンドラーがないActiveなBehavior → Ok(Behavior::same()) を返す
- IgnoreBehavior → メッセージがサイレントに破棄される
- 型チェックエラー → TypedAskError::TypeMismatch を返す

**未処理メッセージのデフォルト処理はない**。
ハンドラー不在の場合のメッセージドロップはsameで継続。
DeadLetterへのログは**送信エラー**時のみ（メールボックス容量超過など）。

実装的には、受信側がメッセージを明示的に処理する責任を持つ（Erlang型）。
