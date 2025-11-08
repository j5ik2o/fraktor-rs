# 要件ドキュメント

## プロジェクト説明 (入力)
references/pekko を参考にして MessageAdapter 機能を作りたい

## Introduction
Pekko Typed の MessageAdapter を cellactor-rs のアクターランタイムへ導入し、外部プロトコルで到着する異なるメッセージ型を既存アクターの型安全なプロトコルへ変換できるアダプタとして提供する。実装方針・API は Pekko Typed の挙動に準拠し、追加機能は導入しない。

## Requirements

### 要件 1: MessageAdapter 登録とライフサイクル
**目的:** As a アクター開発者, I want Pekko Typed と同じスタイルで MessageAdapter を登録し、アクターライフサイクルと一体化させたい。

#### 受入基準
1. When `ActorContext.messageAdapter` または `spawnMessageAdapter` を呼び出したとき, the MessageAdapter サブシステム shall 親アクターの `ActorCell` に `FunctionRef` を追加し、変換クロージャを親アクターと同一スレッドで実行できる `ActorRef` を返却する。
2. While アクター内に複数の変換クロージャが登録されている間, the MessageAdapter サブシステム shall `_messageAdapters` の内部リストで管理し、同一メッセージ型に対する再登録では既存エントリを置き換えることで unbounded growth を防止する。
3. When 最初のアダプタが登録されたとき, the MessageAdapter サブシステム shall 親アクター配下に 1 つの匿名 `FunctionRef` を生成して以後も再利用し、アクター停止時に自動停止して追加リソースを残さない。
4. When アクターが `Behaviors.stopped` などで終了したとき, the MessageAdapter サブシステム shall FunctionRef と `_messageAdapters` の登録情報を解放し、明示的な `stop` 呼び出しを必要としない。

### 要件 2: 型変換とルーティング
**目的:** As a プロトコル設計者, I want Pekko Typed と同様に外部メッセージを型安全にアクターへ渡したい。

#### 受入基準
1. When MessageAdapter の `ActorRef` が外部メッセージを受信したとき, the MessageAdapter サブシステム shall wrap it in `AdaptWithRegisteredMessageAdapter` and deliver it to the parent actor, which scans the `MessageAdapterRegistry` in reverse order and runs only the first adapter whose `TypeId::of::<U>()` matches `envelope.type_id` (TypeId/Any ベースの比較)。
2. When 変換クロージャが呼ばれたとき, the MessageAdapter サブシステム shall 親アクターのスレッド上でクロージャを実行し、アクター内部状態や `context` へ安全にアクセスできるようにする。
3. If いずれの変換クロージャもマッチしない場合, then the MessageAdapter サブシステム shall メッセージを `unhandled` として扱い、通常の DeadLetter ルートへ流す。
4. When `ActorContext.ask` や `spawnMessageAdapter` が `AdaptMessage` を用いて応答を変換する場合, the MessageAdapter サブシステム shall 親アクター側で変換を完了させてからユーザビヘイビアへ渡し、スレッドセーフな単方向変換を保証する。

### 要件 3: 失敗伝播と監督連携
**目的:** As a ランタイム運用者, I want メッセージ変換失敗を Pekko Typed の監督セマンティクスに沿って扱いたい。

#### 受入基準
1. If 変換クロージャが例外を投げた場合, then the MessageAdapter サブシステム shall `MessageAdaptionFailure` シグナルを親アクターへ送信し、デフォルトのシグナルハンドラが例外を再スローして Typed スーパービジョンに渡す。
2. When ユーザビヘイビアが `MessageAdaptionFailure` を明示的に処理する場合, the MessageAdapter サブシステム shall Pekko 同様に `Behaviors.same` などの通常ハンドリングを許容し、監督戦略はその結果に従う。
3. When 親アクターが停止または再起動シーケンスに入ったとき, the MessageAdapter サブシステム shall dispose the `MessageAdapterRegistry` during `Behavior.interpretSignal`’s PostStop/PreRestart handling and keep no adapters alive until再登録される。
