# 機能仕様: Cellactor Actor Core 統合仕様

**作成日**: 2025-10-28  
**ステータス**: Draft  
**入力**: `specs/001-actor-core-basics`, `specs/001-behavior-factory`, `specs/001-dispatcher-async`, `specs/001-event-stream`, `specs/001-mailbox-core`, `specs/001-supervision-errors`, `specs/001-typed-handlers`

## スコープ

- Protoactor-go および Apache Pekko(Akka) Typed を参照した actor-core のコア機能
- Behavior/Props API と型付き・型抹消メッセージ処理の橋渡し
- メールボックス、Dispatcher、MessageInvoker の非同期化とバックプレッシャー制御
- Supervision 戦略と Result ベースのエラー通知モデル
- EventStream の publish/subscribe と観測性
- no_std を前提としたランタイム拡張ポイント（tokio / embassy など）

## 非スコープ

- クラスタリング、リモート PID 解決、分散 Pub/Sub
- 外部 I/O との直接統合（gRPC、HTTP 等）
- メトリクス外部エクスポートやダッシュボード実装

## ユーザーストーリーとテスト（必須）

### ユーザーストーリー1 - ActorSystemでスコープ内アクターを起動する（優先度: P1）
セルアクター利用開発者として、Pekko/Akka と同様に `ActorSystem` に Props/Behaviors を登録し、システム内の `ActorRef` だけでメッセージをやり取りしたい。これにより protoactor-go の `RootContext` を利用した既存コードを Rust では `ActorSystem` のスコープ内コンテキストへ置き換えられる。  
参照: Apache Pekko/Akka Typed `ActorSystem`, `Behaviors.setup`; protoactor-go `actor/root_context.go` との差異として ActorRef をシステム外へ公開しない設計を採用。

**独立テスト**: `ActorSystem::new` でシステムを起動し、`system.spawn(props)` でエコーアクターを生成。`system.run(|ctx| { ... })` 内で `ctx.actor_ref()` を通じて `tell` と `request_future` を実行し、スコープ外から同じ参照を使用できないことを検証する結合テスト。

**受け入れシナリオ**:
1. **前提** `ActorSystem` が起動済みで、`Props::from_behavior` によりエコーアクターを登録。**操作** `system.run` で提供されるアプリケーションスコープ内から `ctx.spawn(props)` を呼び出し、得た `ActorRef` に `tell` を送信。**結果** ハンドラが一度だけ実行され、`ActorResult::Ok` が `ctx` 内で観測できる。
2. **前提** 上記と同一のアクター参照をシステム外へムーブしようとする。**操作** `ActorRef` を `system.run` の外側へ返却して再利用を試みる。**結果** コンパイル時もしくは実行時検証で拒否され、システムスコープ外からはメッセージ送信できないことがログに記録される。

### ユーザーストーリー2 - メールボックスとスケジューリング制御（優先度: P1）
セルアクター利用開発者として、bounded/unbounded mailbox とデフォルト Dispatcher を制御し、負荷時の backpressure と処理順序を一致させたい。  
参照: protoactor-go `mailbox/bounded_mailbox.go`, `actor/dispatcher.go`

**独立テスト**: Bounded mailbox に閾値 +1 のメッセージを投入し backpressure を観測する統合テスト、および Dispatcher のスケジュール順序を検証するプロパティテスト。

**受け入れシナリオ**:
1. **前提** 容量 10 の bounded mailbox を設定した Props。**操作** 11 件のメッセージを連続送信。**結果** 11 件目が保留またはエラーとして通知され、先行 10 件は順序通り完了する。
2. **前提** デフォルト Dispatcher を使用する 3 つの PID。**操作** Round-Robin でメッセージを送信。**結果** スケジューラが protoactor-go と同じ順序で処理を割り当てることがログで確認できる。

### ユーザーストーリー3 - 監視とエラー回復（優先度: P1）
セルアクター運用者として、OneForOne/AllForOne 戦略と Restart/Stop/Resume を利用し、アクター失敗時の回復を制御したい。  
参照: protoactor-go `actor/supervision.go`, `actor/restart_statistics.go`

**独立テスト**: 監視下の子アクターに失敗を発生させ、再起動回数や停止挙動を検証するシナリオテスト。

**受け入れシナリオ**:
1. **前提** ActorSystem のガーディアン Behavior が OneForOne + Restart(最大3回, 60秒窓) を設定済み。**操作** 子アクターへ失敗メッセージを送信。**結果** 3 回まで再起動し、4 回目で停止して親へ通知する。
2. **前提** Stop 戦略を設定。**操作** 子アクターが `Err(ActorError::Fatal)` を返すメッセージを受信。**結果** 子アクターが停止し、監視側へ停止イベントが送出される。

### ユーザーストーリー4 - Behavior ベースでアクターを構築したい（優先度: P1）
セルアクター利用開発者として、Pekko/Akka Typed のように `Behavior<T>` と `Behaviors` ファクトリで受信ロジックを宣言的に定義したい。  
参照: Pekko `Behaviors.setup`, `Behaviors.receiveMessage`

**独立テスト**: Behavior で作成したカウンタアクターを `Props::from_behavior(Behaviors::receive(..))` から起動し、`Tell` を複数回実行する統合テスト。

**受け入れシナリオ**:
1. **前提** `Behaviors::receive` で整数メッセージを加算し `Result<(), ActorError>` を返す Behavior を定義。**操作** `Props::from_behavior(behavior)` で PID を生成し `Tell` を 3 回送信。**結果** Behavior 内部ステートが 3 になり、`RequestFuture` で照会すると `Ok(3)` が返る。
2. **前提** `Behaviors::setup` で初期化クロージャを含む Behavior。**操作** `root.spawn(props)` を呼ぶ。**結果** 初期化が 1 度だけ実行され、後続メッセージ処理は正常に続行する。

### ユーザーストーリー5 - アクターに分かりやすい名前を付けたい（優先度: P1）
運用担当者として、`Props::with_name("order-handler")` などで論理名を設定し、未指定時はシーケンスベースの匿名名を採番して欲しい。  
参照: protoactor-go `actor/process_registry.go`, Pekko Typed の命名規則

**独立テスト**: 名前あり/なしの Props でアクターを生成し、ProcessRegistry に一意な名前で登録されることを検証するテスト。

**受け入れシナリオ**:
1. **前提** `Props::from_behavior(...).with_name("order-handler")`。**操作** ActorSystem のガーディアンコンテキストがアクターを spawn。**結果** PID が `order-handler` 名を保持し、ProcessRegistry で一意に管理される。
2. **前提** 名前未指定の Props。**操作** ActorSystem が 3 つのアクターを順次 spawn。**結果** PID 名が `anonymous-<sequence>` 形式で連番採番され、重複しない。

### ユーザーストーリー6 - Supervision を Behavior と組み合わせたい（優先度: P1）
セルアクター運用者として、Behavior と Supervision 戦略を宣言的に組み合わせ、失敗時の回復ポリシーを制御したい。  
参照: protoactor-go `actor/supervision.go`, Pekko `Behaviors.supervise`

**独立テスト**: 監視下の Behavior が連続で失敗するシナリオで再起動/停止挙動を検証する統合テスト。

**受け入れシナリオ**:
1. **前提** `Behaviors::supervise(behavior).with_restart(max_retries=3, within=60s)` を適用。**操作** 連続して 4 回 `Err(ActorError::Retryable)` を返すメッセージを送る。**結果** 最初の 3 回は再初期化され、4 回目で停止して親に通知される。
2. **前提** Stop 戦略を適用した Behavior。**操作** `Err(ActorError::Fatal)` を返すメッセージを送る。**結果** アクターが停止し、停止イベントが監視側へ通知される。

### ユーザーストーリー7 - FnMut ハンドラで簡易アクターを定義したい（優先度: P1）
セルアクター利用開発者として、`FnMut(&mut ActorContext<T>, T) -> Result<(), ActorError>` 形式のクロージャを直接 Props に渡してアクターを生成したい。  
参照: protoactor-go `actor/props.go`, Pekko Typed `Behaviors.receiveMessage`

**独立テスト**: `Props::from_fn` でインクリメントアクターを生成し、`Tell` を複数回実行してキャプチャされた状態が維持されることを確認する統合テスト。

**受け入れシナリオ**:
1. **前提** FnMut クロージャが内部カウンタをキャプチャし `Ok(())` を返す。**操作** `Props::from_fn(counter_fn)` で spawn し `Tell(1)` を 3 回送信。**結果** 累計値が 3 になり、`RequestFuture` で 3 が返る。
2. **前提** クロージャが子アクター spawn や `RequestFuture` を利用。**操作** 親ハンドラで `Ok(())` を返しつつ子へ派生処理。**結果** コンテキスト API が利用でき、Future が期待値を返す。

### ユーザーストーリー8 - Typed/Untyped メッセージを橋渡ししたい（優先度: P1）
アプリ開発者として、ユーザ向け API は型安全 (`ActorRef<T>`) に保ちつつ、内部では `Any` ベースの Envelope で柔軟にメッセージを扱いたい。  
参照: Pekko Typed `ActorRef<T>`, `akka.actor.ActorRef`

**独立テスト**: 型付き ActorRef にメッセージを送り内部で `Any` へ格納→取り出しまでの流れ、および型不一致時のアダプタ挙動を検証するテスト。

**受け入れシナリオ**:
1. **前提** `ActorRef<String>` を取得。**操作** `Tell("hello")` を実行。**結果** メッセージが内部 `Any` に安全に格納され、ハンドラで `String` として取得できる。
2. **前提** `ActorRef<u32>` に `ActorRef<String>` から直接メッセージを送信。**操作** アダプタ未登録で `Tell("bad")`。**結果** 型不一致エラーとして Dead Letter に転送され、メトリクスに記録される。

### ユーザーストーリー9 - メッセージアダプタでプロトコル変換したい（優先度: P2）
システム統合エンジニアとして、異なるメッセージ型間を変換する MessageAdapter を登録し、受信メッセージを期待型へ変換したい。  
参照: Pekko Typed `ctx.messageAdapter`, protoactor-go `actor/context.go`

**独立テスト**: アダプタ登録済みアクターに異種メッセージを送り、変換後にハンドラが実行される統合テスト。

**受け入れシナリオ**:
1. **前提** `ctx.message_adapter` で外部 `Event` → 内部 `Command` 変換を登録。**操作** 外部 `Event` を送信。**結果** Adapter が `Command` に変換し、ハンドラが処理する。
2. **前提** 複数アダプタを登録。**操作** 異なる型のメッセージを連続送信。**結果** 型一致するアダプタが選択され、未一致は Dead Letter へ転送される。

### ユーザーストーリー10 - utils-core キューを利用したメールボックスが欲しい（優先度: P1）
セルアクター利用開発者として、メールボックスが utils-core の queue 実装を利用し、Props で種別や容量を選択したい。  
参照: protoactor-go `mailbox/bounded_mailbox.go`

**独立テスト**: utils-core bounded queue を利用したメールボックスで 1000 件のメッセージを投入・消費し、順序が保持される統合テスト。

**受け入れシナリオ**:
1. **前提** bounded queue を選択。**操作** 1000 件のユーザーメッセージを送信。**結果** 順序通り処理され欠落や重複がない。
2. **前提** priority queue を選択。**操作** System と User メッセージを交互に送信。**結果** System メッセージが常に優先処理される。

### ユーザーストーリー11 - サスペンド/オーバーフロー制御を行いたい（優先度: P1）
運用担当者として、メールボックスの Suspend/Resume、Stash、オーバーフローポリシーを制御し、ピーク時でも期待どおり動作させたい。  
参照: protoactor-go `mailbox/mailbox.go`, Pekko `Mailbox`/`StashBuffer`

**独立テスト**: 各オーバーフローポリシーに対する挙動とサスペンド/再開の FIFO 動作を検証する結合テスト。

**受け入れシナリオ**:
1. **前提** DropNewest ポリシーで容量 10。**操作** 11 件目を送信。**結果** 最新メッセージが破棄され、ドロップがメトリクスに記録される。
2. **前提** Suspend 中のメールボックス。**操作** `Suspend` 後にメッセージを送信し、その後 `Resume`。**結果** メッセージが Stash に保持され、`Resume` 後 FIFO 順序で再投入される。

### ユーザーストーリー12 - メールボックス観測性を確保したい（優先度: P2）
セルアクター SRE として、メールボックスの throughput、滞留時間、backpressure をメトリクスとして取得し、ミドルウェアチェインへヒントを渡したい。  
参照: Pekko `Mailbox` メトリクス、protoactor-go `monitoring`

**独立テスト**: メトリクスレシーバをモック化し、処理量・滞留・サスペンド時間が記録されることを確認する統合テスト。

**受け入れシナリオ**:
1. **前提** メトリクスミドルウェアを登録。**操作** 1000 件のメッセージを処理。**結果** Throughput と滞留メトリクスが記録され、閾値超過で backpressure アラートが発火する。
2. **前提** サスペンド/再開を繰り返すシナリオ。**操作** 100 回 Suspend/Resume を実行。**結果** Stash サイズとサスペンド時間が記録され、誤差は ±5% 以内。

### ユーザーストーリー13 - 非同期 Dispatcher を差し替えたい（優先度: P1）
セルアクター利用開発者として、no_std 向け Dispatcher と tokio/embassy 向け Dispatcher を用途に応じて差し替えたい。  
参照: Pekko `DispatcherConfigurator`, protoactor-go `DefaultDispatcher`

**独立テスト**: no_std モック Dispatcher と tokio Dispatcher を切り替え、順序とスループットを比較する統合テスト。

**受け入れシナリオ**:
1. **前提** no_std Dispatcher を構成。**操作** 100 メッセージを送信。**結果** 順序通り処理されリソース制約に沿ってスケジュールされる。
2. **前提** tokio Dispatcher を構成。**操作** 同じメッセージパターンを async で送信。**結果** Future が確実に完了し、tokio のタスクモデルと矛盾しない。

### ユーザーストーリー14 - MessageInvoker で async mailbox を統一したい（優先度: P1）
ライブラリ実装者として、Mailbox を含むメッセージ処理パイプラインを async 化し、await ポイントで中断・再開できるようにしたい。  
参照: Pekko `MessageDispatcher`, protoactor-go `MessageInvoker`

**独立テスト**: async メールボックスを介してメッセージを投入し、invoker が await を挟みながら処理するケースを検証。

**受け入れシナリオ**:
1. **前提** backpressure ポリシー DropOldest の async Mailbox。**操作** invoker が await を挟みながら処理する。**結果** 溢れた際に古いメッセージがドロップされ、ログとメトリクスに記録される。
2. **前提** tokio ランタイム下で `MessageInvoker::invoke` が `async fn`。**操作** 1 秒あたり 10,000 通のメッセージを送信。**結果** invoker が Future を完了させ遅延が閾値内に収まる。

### ユーザーストーリー15 - ランタイム間で共通契約を維持したい（優先度: P2）
セルアクター設計者として、Dispatcher/Invoker/メールボックスの組み合わせが no_std・tokio・embassy で破綻しないよう契約テストを整備したい。  
参照: Pekko `Dispatcher` テスト、protoactor-go `scheduler`

**独立テスト**: `cfg` 切り替えで異なる Dispatcher 実装を読み込み、共通契約テストをパスする CI を構築。

**受け入れシナリオ**:
1. **前提** CI で no_std モック構成。**操作** 契約テストを実行。**結果** enqueue/dequeue/await でパニックやデッドロックが起きない。
2. **前提** tokio と embassy を対象に同テストを実行。**結果** いずれの実装も契約を満たし、不整合ログが発生しない。

### ユーザーストーリー16 - EventStream を使って疎結合な通知を送りたい（優先度: P1）
セルアクター利用開発者として、システム全体で共有される EventStream にイベントを publish し、型に基づく購読を行いたい。  
参照: protoactor-go `eventstream/event_stream.go`, Pekko `EventStream`

**独立テスト**: publish→型一致購読者のみ通知される統合テスト。

**受け入れシナリオ**:
1. **前提** `SystemEvent` を購読するアクター登録済み。**操作** `EventStream.publish(Box::new(SystemEvent::Started))` を呼ぶ。**結果** 該当購読者のコールバックが 1 回呼ばれる。
2. **前提** 別のアクターが `UserEvent` のみ購読。**操作** `SystemEvent` を publish。**結果** `UserEvent` 購読者には通知されない。

### ユーザーストーリー17 - 一時的な購読と解除を管理したい（優先度: P1）
オペレーション担当者として、一定期間だけ購読したり、アクター停止時に自動解除したい。  
参照: protoactor-go `eventstream.Subscription`, Pekko `EventStream.subscribe/unsubscribe`

**独立テスト**: `subscribe`→`unsubscribe` の流れで通知が止まること、アクター停止時に自動解除されることを確認するテスト。

**受け入れシナリオ**:
1. **前提** Actor A が購読中。**操作** `unsubscribe(A)` の後イベント publish。**結果** A のコールバックは呼ばれない。
2. **前提** Actor B が購読中。**操作** Actor B を停止。**結果** EventStream の購読リストから B が除去され以後通知されない。

### ユーザーストーリー18 - イベントストリームの観測性とバッファ制御が欲しい（優先度: P2）
セルアクター SRE として、EventStream の publish/subscribe 数やドロップ数を観測し、遅延購読者にバッファ戦略を適用したい。  
参照: Pekko EventStream 監視 API

**独立テスト**: 遅延購読者をシミュレートし、設定したドロップ戦略とメトリクス記録が期待通り動作することを確認。

**受け入れシナリオ**:
1. **前提** バッファ容量 10、DropOldest 戦略を設定。**操作** 15 件のイベントを publish、購読者は処理を遅延。**結果** 古い 5 件がドロップされ、ドロップ数メトリクスに反映される。
2. **前提** メトリクス監視が有効。**操作** 1000 件のイベントを publish。**結果** Throughput と遅延指標が記録され、閾値超過時にバックプレッシャー警告が生成される。

### ユーザーストーリー19 - Result ベースでエラー通知を受け取りたい（優先度: P1）
セルアクター利用開発者として、ハンドラや Behavior が `Result<(), ActorError>` を返し、`Err` がスーパーバイザへ通知される仕組みを使いたい。  
参照: Pekko `SupervisorStrategy`, protoactor-go `actor/process.go`

**独立テスト**: ハンドラが `Err(ActorError::Transient)` を返すケースで親スーパーバイザが再起動を選択する統合テスト。

**受け入れシナリオ**:
1. **前提** 子アクターが `Result<(), ActorError>` を返す Behavior。**操作** エラーを誘発し `Err(ActorError::Retryable)` を返す。**結果** 親がエラー詳細を受信し、戦略に従って Restart する。
2. **前提** 同構成で `Err(ActorError::Fatal)` を返す。**操作** エラー発生メッセージを送信。**結果** 親が Stop 戦略を適用し子アクターが停止する。

### ユーザーストーリー20 - panic を契約違反として扱いたい（優先度: P1）
運用担当者として、アクターハンドラで panic が発生した場合は自動復旧せず停止扱いにしたい。  
参照: Pekko `SupervisorStrategy`, Rust panic handling

**独立テスト**: ハンドラ内で `panic!()` を呼び、スーパービジョンが再起動を試みないことを確認するテスト。

**受け入れシナリオ**:
1. **前提** Restart 戦略を設定した親。**操作** 子アクターで panic を発生。**結果** 子が停止し、再起動は試行されず致命イベントが通知される。
2. **前提** actor-std 拡張で panic リカバリオプションが有効。**操作** 同様に panic。**結果** 利用者が選択した場合のみ catch_unwind でリカバリ可能だがデフォルトでは停止する。

### ユーザーストーリー21 - エラー種別ごとに対応を変えたい（優先度: P2）
セルアクター設計者として、`ActorError` に再起動可否や再試行ポリシーを持たせ、戦略が Restart/Resume/Stop/Escalate を判断できるようにしたい。  
参照: Pekko `SupervisorStrategy.Decider`, protoactor-go `actor/supervision.go`

**独立テスト**: `ActorError` の種別に応じて Restart/Resume/Stop を選択するシナリオテスト。

**受け入れシナリオ**:
1. **前提** Decider が `ActorError::Retryable` を Restart にマップ。**操作** `Err(ActorError::Retryable)` を返す。**結果** 再起動が実行され統計が更新される。
2. **前提** Decider が `ActorError::Ignore` を Resume にマップ。**操作** `Err(ActorError::Ignore)` を返す。**結果** アクターは停止せず処理を継続し、監視イベントに履歴が残る。

## 要件（必須）

### Actor システム基盤
- **FR-ACT-001**: ActorSystem は Pekko Typed と同様にガーディアン Behavior を受け取り、Props/Behaviors をシステム内部で spawn できなければならない。protoactor-go の RootContext API を移植する場合は ActorSystem 内部の SystemContext にマッピングし、外部公開しない。
- **FR-ACT-002**: `Tell`/`Request`/`RequestFuture` を提供し、Envelope に送信者 PID・ヘッダー・レスポンスチャネルを保持しなければならない。これらは SystemContext から取得した `ActorRef` 経由でのみ呼び出せること。
- **FR-ACT-003**: Watch/Unwatch、停止通知、Dead Letter 転送を備え、protoactor-go `actor/watch.go` と同等に親子ライフサイクルを扱わなければならない。監視 API も ActorSystem 内部のスコープでのみ利用可能とする。
- **FR-ACT-004**: `ActorRef` または PID が無効・システム外スコープの場合は Result でエラーを返却し、呼び出し側が再送/破棄を選択できなければならない。スコープ外利用は検知されログに記録されること。
- **FR-ACT-005**: ActorSystem は `run`/`block_on` などのエントリポイントを通じてアプリケーションコードをシステム内スコープに入れ、完了後は全 `ActorRef` を破棄する。これにより ActorSystem 終了後に `ActorRef` を使用できないよう保証し、Pekko Typed の `ActorSystem/ActorContext` と同等の安全域を提供する。

### Behavior / Props / ハンドラ
- **FR-BEH-001**: `Behavior<T>` 抽象を提供し、メッセージ処理・初期化・終了フックを純粋関数として表現できなければならない。デフォルト実装はジェネリックを用いてトレイトオブジェクト多用を避けること。
- **FR-BEH-002**: `Behaviors` ファクトリ（`setup`, `receive`, `receive_message`, `with_stash`, `supervise` など）を提供し、Props 構築を簡潔に記述できなければならない。
- **FR-BEH-003**: Props は `Props::from_behavior` と `Props::from_fn` の 2 系列コンストラクタを提供し、いずれも内部で共通ビルダ API を共有しなければならない。
- **FR-BEH-004**: `Props::with_name` で指定名を設定し、未指定時は `anonymous-<sequence>` 形式で deterministic に採番しなければならない。
- **FR-BEH-005**: ハンドラ/Behavior は `Result<(), ActorError>` を返し、`Ok(())` で継続、`Err` でスーパーバイザ通知を行わなければならない。
- **FR-BEH-006**: MessageAdapter を登録し、外部型 `U` を内部型 `T` に変換できなければならない。複数登録・型一致検索・未一致時の Dead Letter をサポートすること。

### メールボックス / Dispatcher / Invoker
- **FR-MBX-001**: メールボックスは utils-core の queue（bounded/unbounded/priority）を利用し、Props で選択可能にしなければならない。
- **FR-MBX-002**: メールボックスは System メッセージを User メッセージより高優先度で処理しなければならない。
- **FR-MBX-003**: Suspend/Resume と Stash バッファを提供し、サスペンド中のメッセージを FIFO で再投入しなければならない。
- **FR-MBX-004**: DropNewest/DropOldest/Grow/Block のオーバーフローポリシーをサポートし、それぞれに対応した監視イベントとメトリクスを記録しなければならない。
- **FR-MBX-005**: メールボックス処理は Inbound/Outbound ミドルウェアチェインを経由し、backpressure ヒントを伝播できなければならない。
- **FR-MBX-006**: Dispatcher 抽象を提供し、no_std（同期）と tokio/embassy（async）実装を差し替え可能にしなければならない。
- **FR-MBX-007**: MessageInvoker は `async fn invoke` を提供し、Mailbox から取り出したメッセージを await 可能に処理しなければならない。no_std ではポーリング fallback を用意すること。
- **FR-MBX-008**: Dispatcher/Invoker/メールボックスの契約テストを共通化し、異なるランタイム実装でも同一の期待を満たさなければならない。

### EventStream
- **FR-EVT-001**: グローバル EventStream を提供し、`publish<E>()` で任意イベントを送信できなければならない。
- **FR-EVT-002**: 型ベースの `subscribe<E>`/`unsubscribe<E>` を提供し、アクター停止時に自動解除すること。
- **FR-EVT-003**: 内部キューは utils-core の queue を利用し、メールボックスと同等のオーバーフローポリシーを設定できなければならない。
- **FR-EVT-004**: 遅延購読者へ BackpressureHint を発行し、必要に応じてドロップまたは遅延制御を行わなければならない。
- **FR-EVT-005**: publish/ドロップ/購読登録・解除をメトリクスとログに記録しなければならない。

### Supervision / エラー処理
- **FR-SUP-001**: Supervision 戦略は Restart/Stop/Resume/Escalate をサポートし、`ActorError` の内容と統計に基づき判定できなければならない。
- **FR-SUP-002**: `ActorError` は再試行回数・時間窓などのメタ情報を保持し、Decider が利用できなければならない。
- **FR-SUP-003**: panic は契約違反として扱い、actor-core では自動復旧を試みず停止扱いとしなければならない。actor-std のみ catch_unwind をオプション提供する。
- **FR-SUP-004**: エラー通知・panic 通知をメトリクス/ログへ記録し、再試行・停止回数を分析できるようにすること。
- **FR-SUP-005**: エラー時にメールボックス内メッセージが消失しないよう状態遷移を整合させなければならない。

### Typed/Untyped 橋渡し
- **FR-TYP-001**: `ActorRef<T>` と `ActorContext<T>` を型安全に公開しながら、内部では `UntypedEnvelope`（`Any` ベース）で処理しなければならない。
- **FR-TYP-002**: 型変換失敗時は Dead Letter とメトリクス記録を行い、監視者へ通知しなければならない。

### プラットフォーム/命名規約
- **FR-PLT-001**: すべてのコア機能は `#![no_std]` でビルド可能であり、`std` 依存は `cfg(test)` または `*-std`/`*-embedded` クレートに隔離しなければならない。
- **FR-PLT-002**: 共有参照やロックは `modules/utils-core` の `Shared`/`ArcShared`/`RcShared` と `AsyncMutexLike`/`SyncMutexLike` 抽象を利用し、直接 `alloc::sync::Arc` などへ依存してはならない。
- **FR-PLT-003**: `Shared` 系の型・変数名には `_shared`/`Shared` サフィックスを付与し、`Handle` プレフィックス/サフィックスは使用しない。
- **FR-PLT-004**: 通信・シリアライズ・トランスポート層は抽象インターフェイスを介し、特定実装に固定しない。

## 境界条件・例外
- Bounded mailbox の容量 0 は設定エラーとし、spawn 前に失敗を返す。
- no_std 環境で時間ベースのタイムアウトが提供できない場合は utils-core の時間抽象によるポーリングフォールバックを使用する。
- 名前付きアクターで重複名を指定した場合は自動的に `name-<sequence>` を付与し、一意性と警告ログを提供する。
- MessageAdapter が未登録で型不一致が発生した場合は Dead Letter に転送し、監視イベントとメトリクスを発火する。
- panic は actor-core では停止扱いだが、actor-std で catch_unwind を有効化した場合のみ利用者がリカバリ可能とする。
- ActorSystem スコープ外へ `ActorScopeRef`/PID をムーブした場合は検証フェーズで拒否し、ログに警告を残す。

## 重要エンティティ

- **ActorSystem**: ガーディアン Behavior、ProcessRegistry、Dispatcher を保持しアクター登録/停止を管理。アプリケーションコードをシステムスコープへ入退場させる。
- **SystemContext**: ActorSystem 内で提供されるスコープ付きコンテキスト。アクター生成、メッセージ送信、監視制御を担い、外部へ漏洩しない。
- **Props**: Behavior/FnMut/監視/メールボックス/ミドルウェア設定を束ねるビルダ。`from_behavior`/`from_fn`/`with_*` チェーンを提供。
- **Behavior<T>**: メッセージ型 T を処理し次状態を返す純粋関数。Result でエラー通知を行う。
- **ActorScopeRef<T>**: SystemContext から取得する型付きハンドル。内部で PID と AdapterRegistry を保持し、`Send`/`Sync` を必要最低限に限定したうえでシステムスコープ内でのみ有効となる。
- **ActorContext<T>**: 型付き API と内部 `UntypedEnvelope` 変換を担う。
- **UntypedEnvelope**: `Any` メッセージと送信者 PID、ヘッダーを保持するコンテナ。
- **ActorError**: 再試行ポリシー、時間窓、重篤度を保持するエラー型。
- **SupervisionStrategy/SupervisionDecider**: エラー入力に基づき Restart/Stop/Resume/Escalate を返す判定器。
- **Mailbox / MailboxConfig / MailboxMetrics / StashBuffer**: メッセージキュー、優先度、オーバーフローポリシー、観測情報を管理。
- **Dispatcher / DispatcherConfig / DispatcherMetrics**: メッセージ処理スケジュールと観測を管理し、ランタイム毎に実装差し替え可能。
- **MessageInvoker**: Mailbox から取り出したメッセージを async に処理する実体。
- **EventStream / Subscription / EventBuffer / EventMetrics / BackpressureHint**: publish/subscribe、バッファリング、観測、遅延通知を担当。
- **AdapterRegistry / MessageAdapter**: 型変換アダプタを登録・解決するテーブル。

## 成功指標（必須）

- **SC-001**: ActorSystem スコープ内から `tell` したメッセージの 95% が 5ms 以内に処理完了する（ホスト/組込み双方）。
- **SC-002**: Bounded mailbox で容量超過時のエラー通知または保留制御が 100% 実行され、メッセージ消失が発生しない。
- **SC-003**: 監視戦略テストで再起動/停止動作が期待通り遷移し、意図しない再起動が 0 件。
- **SC-004**: Behavior API で実装したカウンタサンプルの移植時間が protoactor-go 版と比較して 20% 以内の差で完了する（ヒアリング評価）。
- **SC-005**: 名前未指定アクターを 10,000 個連続生成しても PID 名の衝突が 0 件。
- **SC-006**: メールボックスが 10,000 msg/s を処理してもドロップ率 0%、平均待ち時間 2ms 以下を維持する。
- **SC-007**: DropNewest/DropOldest/Grow/Block の各ポリシーで期待挙動とイベント記録が 100% 一致する。
- **SC-008**: メールボックスサスペンド/再開 100 回で FIFO 順序破壊が 0 件、サスペンド時間観測誤差 ±5% 以内。
- **SC-009**: tokio Dispatcher で 10,000 msg/s を処理した際の平均レイテンシが 5ms 以下、ドロップ率 0%。
- **SC-010**: no_std Dispatcher でも 1,000 msg/s 処理でパニックやデッドロックが発生せず、CPU 利用率が想定範囲内（±10%）。
- **SC-011**: Dispatcher/Invoker/メールボックス共通契約テストを tokio / embassy / no_std の 3 構成で実行しエラー 0。
- **SC-012**: EventStream で 1000 件/秒 publish 時、平均遅延が 5ms 以下、ドロップ戦略が 100% 適用される。
- **SC-013**: EventStream 購読解除後にイベントが配送されるケースが 0 件。
- **SC-014**: Result ベースのエラー処理により Retryable エラー再起動成功率が 95% 以上、panic で再起動が試行されるケースが 0 件。
- **SC-015**: 型不一致テスト 100 件で Dead Letter もしくはアダプタ変換が 100% 正しく動作し、未処理メッセージが発生しない。

## 前提・制約

- 参照元は protoactor-go v1 系 `actor`/`mailbox`/`eventstream` と Apache Pekko 1.0 Typed API。差分は仕様で明示し、根拠を残す。
- actor-core ではジェネリックを優先し、トレイトオブジェクトを使用する場合は性能とコードサイズ影響を分析して記録する。
- 自動採番は utils-core の決定的カウンタ/乱数抽象に依存し、時刻非依存とする。
- 監視戦略のテレメトリ出力は actor-core ではイベント発火に留め、外部メトリクス送信は `actor-std` 拡張で扱う。
- no_std における時間管理は utils-core のタイマー抽象に依存し、新たなタイマー機能を actor-core に追加しない。
- メトリクスの外部公開やダッシュボードは将来の observability 仕様で扱う。

## 非目標

- Behavior DSL のマクロ化やコード生成などのシンタックスシュガー。
- リモート/クラスタリング環境での命名解決や EventStream レプリケーション。
- メッセージスキーマ自動生成、リフレクションベース検証。
- tokio/embassy 以外の async ランタイム（async-std 等）への公式サポート。
- メッセージ優先度を System/User 以外の多段階に拡張すること。
