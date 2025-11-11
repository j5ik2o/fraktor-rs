# 要件ドキュメント

## Introduction
Mailbox と Dispatcher を Pekko 互換仕様へリファクタリングし、システムメッセージ優先度、バックプレッシャー、ディスパッチャスケジューリング、観測性のいずれも no_std/STD 共通 API で保証する。

## Requirements

### Requirement 1: システムメッセージ優先 Mailbox
**Objective:** ランタイム開発者として、Pekko 同等のシステムメッセージ優先順序を Mailbox で強制したい。

#### Acceptance Criteria
1. When SystemMailbox に `SystemMessage::Create`/`Recreate`/`Failure` が投入されるとき、the Mailbox shall 常にユーザメッセージより先にそれらをデキューする。
2. When ユーザ Mailbox が空で SystemMailbox のみ待機しているとき、the Mailbox shall ディスパッチャスレッドが遊休でもシステムメッセージを即時処理する。
3. If Mailbox デキュー処理でシステムメッセージとユーザメッセージが同時検出された場合、then the Mailbox shall システムメッセージを優先し、ユーザメッセージを再度エンキューして順序を保つ。
4. While Mailbox が PriorityClass=System を処理中である間、the Dispatcher shall 他アクターのユーザメッセージ取得を遅延させない。
5. The Mailbox shall Pekko の system message ordering 仕様と同一の優先度リスト（Create→Recreate→Watch/Unwatch→Terminate）を保持する。
6. When systemEnqueue/systemDrain が同時実行されるとき、the Mailbox shall lock-free CAS によりシステムメッセージリストを更新し、失敗時は unlink→retries を行い、閉塞時は DeadLetter へフォールバックする。

### Requirement 2: Dispatcher スケジューリング互換
**Objective:** ランタイム運用者として、Pekko Dispatcher と同等のスループット/フェアネス調整を Rust ランタイムで利用したい。

#### Acceptance Criteria
1. When Dispatcher がアクターをフェッチするとき、the Dispatcher shall 1 ループあたりのメッセージ処理件数（throughput）を設定可能にする。
2. When throughput-limited ループが終了したとき、the Dispatcher shall Mailbox 残量がある場合は再スケジュールし、無い場合はワーカースレッドを返却する。
3. When registerForExecution に hasMessageHint/hasSystemMessageHint が渡されるとき、the Dispatcher shall Pekko と同じヒントロジックで canBeScheduledForExecution を判定し Register を 1 回だけ実行する。
4. If executor が `RejectedExecution` を返した場合、then the Dispatcher shall 2 回まで再投入を試行し失敗時は Mailbox を Idle に戻して EventStream へエラーを通知する。
5. If Dispatcher が starvation を検出する負荷計測（一定時間メッセージ未処理）を満たす場合、then the Dispatcher shall 新規ワーカーをスポーンもしくは既存ワーカーを再割り当てする。
6. While no_std 実行環境において thread 概念が無い間、the Dispatcher shall Tick ベースの Cooperative スケジューラ API にフォールバックする。
7. The Dispatcher shall Pekko の `Dispatcher` 設定項目（throughput, throughput-deadline-time, mailbox-type）の論理設定を解釈できる。

### Requirement 3: Mailbox バックプレッシャーと容量管理
**Objective:** プラットフォーム開発者として、mailbox オーバーフロー時の安全なバックプレッシャー制御を保証したい。

#### Acceptance Criteria
1. When Mailbox のエンキュー要求が容量上限に達したとき、the Mailbox shall 指定された OverflowStrategy（DropHead/DropNew/Fail/DeadLetter/Block）を実行する。
2. If OverflowStrategy=Fail の場合にエンキューが拒否されたとき、then the Mailbox shall 呼び出し元へ明示的な `Result::Err` を返し、DeadLetter へ転送しない。
3. When Mailbox が backpressure シグナルを発火したとき、the Dispatcher shall 当該アクターのスケジューリング頻度を減少させる。
4. While Mailbox 容量が 75% を超えている間、the ActorSystemGeneric shall EventStream へ `MailboxPressure` イベントを配信する。
5. The Mailbox shall no_std/STD いずれのビルドでも同一容量設定が適用される構成 API を提供する。
6. When OverflowStrategy=Block が選択され push timeout が設定されているとき、the Mailbox shall Pekko `BoundedMailbox` と同等に送信側を非同期 Future で待機させ、タイムアウト時は DeadLetter 通知と `Result::Err` を返す。
7. While actors declare `Stash`/`RequiresMessageQueue<Deque>` を利用する間、the Mailbox shall Deque ベース（両端 enqueue/dequeue）API を公開し stash/un-stash 操作がロスなく往復できるようにする。
8. If an actor enabling Stash 要件が Deque 対応 Mailbox を得られない場合、then the ActorSystemGeneric shall 初期化を失敗させ、必要なキュー種別と選択された Mailbox をエラーに含める。
9. When Stash capacity が RuntimeConfig API から提供されるとき、the ActorSystemGeneric shall この容量を超える stash 操作で `StashOverflow` 相当のエラーを返し、未処理メッセージを DeadLetter へ転送する。

### Requirement 4: Telemetry & DeadLetter 観測性
**Objective:** オブザーバビリティ担当として、Mailbox/Dispatcher の状態を Pekko 互換の指標で計測したい。

#### Acceptance Criteria
1. When Mailbox 深さや Dispatcher 負荷メトリクスが更新されるとき、the ActorSystemGeneric shall EventStream にメトリクスサンプルを発行する。
2. If DeadLetter が Mailbox オーバーフローや Dispatch 失敗で発生した場合、then the DeadLetter Service shall 原因（overflow, scheduler-failure, shutdown）を属性に含める。
3. While Telemetry サブスクライバが未登録である間、the ActorSystemGeneric shall メトリクス収集を最小オーバーヘッドで維持し、不要なバッファを確保しない。
4. When 運用者が Pekko 形式の dispatcher dump 要求を発行するとき、the ActorSystemGeneric shall アクターごとのキュー長とアサイン済みワーカー ID を報告する。
5. The EventStream shall no_std/STD 共通の Subscriber API でこれらのテレメトリイベントを受信できる。

### Requirement 5: Mailbox 自己スケジューリング状態機械
**Objective:** ランタイム開発者として、Mailbox 自身が Pekko と同等の Scheduled/Suspended/Running 状態を制御できるようにしたい。

#### Acceptance Criteria
1. When Mailbox が Idle 状態で初回エンキューを受け取るとき、the Mailbox shall 自身を Scheduled へ遷移させ単一の registerForExecution を Dispatcher に要求する。
2. When Dispatcher が Mailbox 実行を開始するとき、the Mailbox shall Running 状態で poll_once 相当の API を提供し NeedReschedule/Idle/Closed の実行結果を返す。
3. If Mailbox がシステムキューとユーザキューを空にした場合、then the Mailbox shall 即座に Idle 状態へ戻し dispatcher.registerForExecution(..., false, false) を再要求して race を防止する。
4. While Mailbox が Suspended 状態である間、the Dispatcher shall システムメッセージのみ処理を許可しユーザメッセージ配送を抑止する。
5. When Mailbox 実行ループが完了したとき、the Mailbox shall processAllSystemMessages→processMailbox の順で drain し、その後 setAsIdle→DeadLetter drain→状態完了イベントを一連で実行する。
6. If Mailbox が Closed へ遷移した場合、then the Mailbox shall 残余メッセージを DeadLetter へ drain し状態完了イベントを発行する。

### Requirement 6: API 駆動 Dispatcher/Mailbox 解決
**Objective:** オペレーション担当として、Props や構成 API で受け取った dispatcher/mailbox 指定を Pekko と同等に差し替えたい。

#### Acceptance Criteria
1. When Props or RuntimeConfig API が dispatcher ID を提供するとき、the Dispatcher shall API 由来の DispatcherDescriptor から一致する Dispatcher を生成し Actor へ割り当てる。
2. When Props or Deploy API が mailbox ID を上書きするとき、the Mailboxes Service shall Pekko と同じ優先順位（Props.deploy.mailbox→dispatcher descriptor の mailbox-type→デフォルト）で解決する。
3. If Props or DispatcherDescriptor API が未知の dispatcher ID または mailbox type を参照した場合、then the ActorSystemGeneric shall Actor 作成を失敗させエラーレスポンスに該当識別子を含める。
4. When Props が `RequiresMessageQueue` 相当の要件トレイトを宣言するとき、the Mailboxes Service shall `ProducesMessageQueue` 情報を検証し不一致なら `Result::Err` を返して Actor 作成を失敗させる。
5. While Pekko 互換モードが有効である間、the Dispatcher shall API で渡された Pekko 互換属性（throughput, throughput-deadline-time, mailbox-requirement）を読み替えて同等の意味で適用する。
6. The Mailboxes Service shall no_std/STD 共通の Builder/API で容量・overflow・suspend 戦略を宣言的に上書きできる。

### Requirement 7: STD 実行器ブリッジ互換
**Objective:** ランタイム統合担当として、Tokio などのホスト実行器上で Pekko と同じ Mailbox/Dispatcher 契約を保ちたい。

#### Acceptance Criteria
1. When STD 実行環境で Dispatcher が Mailbox を再スケジューリングするとき、the Dispatcher shall ホスト実行器ハンドルへ非同期タスクとして登録し busy-wait を禁止する。
2. If Mailbox の poll_once が NeedReschedule を返す場合、then the Dispatcher shall ホスト実行器の waker/notify を用いて同じ Mailbox の再実行を要求する。
3. While Mailbox が Pending 状態で新規メッセージ到着を待っている間、the Mailbox shall ホスト実行器互換の waker インターフェースを公開しスピンループなしで待機する。
4. If ホスト実行器が `RejectedExecution` を返した場合、then the Dispatcher shall 2 回まで再投入をリトライし、失敗時は Mailbox を Idle へ戻して EventStream にエラーを publish する。
5. When ホスト実行器が利用不能または過負荷を検出するとき、the Dispatcher shall Mailbox を内部キューへ退避し EventStream へ backpressure 指標を発行する。
6. The no_std Runtime shall 同一の Mailbox/Dispatcher 契約を維持しつつホスト実行器ブリッジをバイパスする。

### Requirement 8: Mailbox Future Handshake
**Objective:** メッセージ送受信パス担当として、Mailbox と Dispatcher の非同期ハンドシェイク（offer/poll future）を必須化したい。

#### Acceptance Criteria
1. When Mailbox の enqueue が `EnqueueOutcome::Pending` を返すとき、the Mailbox shall `MailboxOfferFutureGeneric` ハンドルを提供し、dispatcher/sender が完了まで待機できるようにする。
2. When Dispatcher or DispatcherSender drains a MailboxOfferFuture, the Dispatcher shall `ScheduleWaker` を用いて自分自身を再スケジュールする waker を生成する。
3. If MailboxOfferFuture の poll が `Poll::Pending` を返した場合、then the Dispatcher shall 即座に `schedule()` を呼び出し、その間は `block_hint` 等の軽量スピンのみで待機して busy-wait を避ける。
4. When runtime components need to await the next user message, the Mailbox shall `MailboxPollFutureGeneric` を提供し、STD ブリッジではホスト実行器の waker へ変換して利用できるようにする。
5. If MailboxOfferFuture または MailboxPollFuture が `Result::Err` で完了した場合、then the Dispatcher shall 呼び出し元へ同じエラーを返し、必要に応じて DeadLetter/EventStream へ損失を報告する。

### Requirement 9: utils-core Queue Capabilities
**Objective:** ランタイム共通基盤担当として、Mailbox/Dispatcher が要求する RingBuffer/Deque 機能を `utils-core` 側で保証したい。

#### Acceptance Criteria
1. When Mailbox が MPSC キューを生成するとき、the utils-core queue shall VecRingStorage ベースの lock-free MPSC offer/poll と OverflowPolicy=Block/Grow/Drop を提供する。
2. When OverflowStrategy=Block を使用するとき、the utils-core queue shall QueueOfferFuture/QueuePollFuture を通じて非同期 wait ハンドルを生成できる。
3. While actors 宣言的に `RequiresMessageQueue<Deque>` を適用している間、the utils-core queue shall Deque ベースの push_front/pop_front を公開する拡張（もしくは deque-capable backend）を提供し、stash/un-stash をゼロコピーで実現できるようにする。
4. If utils-core queue backend が要件を満たさない場合、then the Mailbox shall ビルドを失敗させ、欠落機能（Deque/BlockingFuture 等）をエラーに含める。
