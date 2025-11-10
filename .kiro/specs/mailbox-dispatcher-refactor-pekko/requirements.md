# 要件ドキュメント

## Introduction
Mailbox と Dispatcher を Pekko 互換仕様へリファクタリングし、システムメッセージ優先度、バックプレッシャー、ディスパッチャスケジューリング、観測性のいずれも no_std/STD 共通 API で保証する。

## Requirements

### Requirement 1: システムメッセージ優先 Mailbox
**Objective:** ランタイム開発者として、Pekko 同等のシステムメッセージ優先順序を Mailbox で強制したい。

#### Acceptance Criteria
1. When SystemMailbox に `SystemMessage::Create`/`Recreate`/`Failure` が投入されるとき、the Actor Runtime shall 常にユーザメッセージより先にそれらをデキューする。
2. When ユーザ Mailbox が空で SystemMailbox のみ待機しているとき、the Actor Runtime shall ディスパッチャスレッドが遊休でもシステムメッセージを即時処理する。
3. If Mailbox デキュー処理でシステムメッセージとユーザメッセージが同時検出された場合、then the Mailbox shall システムメッセージを優先し、ユーザメッセージを再度エンキューして順序を保つ。
4. While Mailbox が PriorityClass=System を処理中である間、the Dispatcher shall 他アクターのユーザメッセージ取得を遅延させない。
5. The Mailbox shall Pekko の system message ordering 仕様と同一の優先度リスト（Create→Recreate→Watch/Unwatch→Terminate）を保持する。

### Requirement 2: Dispatcher スケジューリング互換
**Objective:** ランタイム運用者として、Pekko Dispatcher と同等のスループット/フェアネス調整を Rust ランタイムで利用したい。

#### Acceptance Criteria
1. When Dispatcher がアクターをフェッチするとき、the Dispatcher shall 1 ループあたりのメッセージ処理件数（throughput）を設定可能にする。
2. When throughput-limited ループが終了したとき、the Dispatcher shall Mailbox 残量がある場合は再スケジュールし、無い場合はワーカースレッドを返却する。
3. If Dispatcher が starvation を検出する負荷計測（一定時間メッセージ未処理）を満たす場合、then the Dispatcher shall 新規ワーカーをスポーンもしくは既存ワーカーを再割り当てする。
4. While no_std 実行環境において thread 概念が無い間、the Dispatcher shall Tick ベースの Cooperative スケジューラ API にフォールバックする。
5. The Dispatcher shall Pekko の `Dispatcher` 設定項目（throughput, throughput-deadline-time, mailbox-type）の論理設定を解釈できる。

### Requirement 3: Mailbox バックプレッシャーと容量管理
**Objective:** プラットフォーム開発者として、mailbox オーバーフロー時の安全なバックプレッシャー制御を保証したい。

#### Acceptance Criteria
1. When Mailbox のエンキュー要求が容量上限に達したとき、the Mailbox shall 指定された OverflowStrategy（DropHead/DropNew/Fail/DeadLetter）を実行する。
2. If OverflowStrategy=Fail の場合にエンキューが拒否されたとき、then the Actor Runtime shall 呼び出し元へ明示的なエラーを返し、DeadLetter へ転送しない。
3. When Mailbox が backpressure シグナルを発火したとき、the Dispatcher shall 当該アクターのスケジューリング頻度を減少させる。
4. While Mailbox 容量が 75% を超えている間、the Actor Runtime shall EventStream へ `MailboxPressure` イベントを配信する。
5. The Mailbox shall no_std/STD いずれのビルドでも同一容量設定が適用される構成 API を提供する。

### Requirement 4: Telemetry & DeadLetter 観測性
**Objective:** オブザーバビリティ担当として、Mailbox/Dispatcher の状態を Pekko 互換の指標で計測したい。

#### Acceptance Criteria
1. When Mailbox 深さや Dispatcher 負荷メトリクスが更新されるとき、the Actor Runtime shall EventStream にメトリクスサンプルを発行する。
2. If DeadLetter が Mailbox オーバーフローや Dispatch 失敗で発生した場合、then the DeadLetter Service shall 原因（overflow, scheduler-failure, shutdown）を属性に含める。
3. While Telemetry サブスクライバが未登録である間、the Actor Runtime shall メトリクス収集を最小オーバーヘッドで維持し、不要なバッファを確保しない。
4. When 運用者が Pekko 形式の dispatcher dump 要求を発行するとき、the Runtime shall アクターごとのキュー長とアサイン済みワーカー ID を報告する。
5. The EventStream shall no_std/STD 共通の Subscriber API でこれらのテレメトリイベントを受信できる。

### Requirement 5: Mailbox 自己スケジューリング状態機械
**Objective:** ランタイム開発者として、Mailbox 自身が Pekko と同等の Scheduled/Suspended/Running 状態を制御できるようにしたい。

#### Acceptance Criteria
1. When Mailbox が Idle 状態で初回エンキューを受け取るとき、the Mailbox shall 自身を Scheduled へ遷移させ単一の registerForExecution を Dispatcher に要求する。
2. When Dispatcher が Mailbox 実行を開始するとき、the Mailbox shall Running 状態で poll_once 相当の API を提供し NeedReschedule/Idle/Closed の実行結果を返す。
3. If Mailbox がシステムキューとユーザキューを空にした場合、then the Mailbox shall 即座に Idle 状態へ戻し Dispatcher ワーカーを解放する。
4. While Mailbox が Suspended 状態である間、the Dispatcher shall システムメッセージのみ処理を許可しユーザメッセージ配送を抑止する。
5. If Mailbox が Closed へ遷移した場合、then the Mailbox shall 残余メッセージを DeadLetter へ drain し状態完了イベントを発行する。

### Requirement 6: Config 駆動 Dispatcher/Mailbox 解決
**Objective:** オペレーション担当として、Props や設定ファイルから Pekko と同等の dispatcher/mailbox を差し替えたい。

#### Acceptance Criteria
1. When Props が dispatcher ID を指定するとき、the Dispatchers Service shall 設定ツリーから一致する Dispatcher を生成し Actor へ割り当てる。
2. If Props が未知の dispatcher ID または mailbox type を参照した場合、then the Actor Runtime shall Actor 作成を失敗させ具体的な設定キーを含むエラーを返す。
3. When Props が mailbox 要件 trait を宣言するとき、the Mailboxes Service shall 互換する Mailbox 実装のみをバインドする。
4. While Pekko 互換モードが有効である間、the Dispatchers Service shall `akka.actor.*.dispatcher` および `mailbox` キーを読み替えて同等の設定値を適用する。
5. The Mailboxes Service shall no_std/STD 共通の設定 API で容量・overflow・suspend 戦略を宣言的に上書きできる。

### Requirement 7: STD 実行器ブリッジ互換
**Objective:** ランタイム統合担当として、Tokio などのホスト実行器上で Pekko と同じ Mailbox/Dispatcher 契約を保ちたい。

#### Acceptance Criteria
1. When STD 実行環境で Dispatcher が Mailbox を再スケジューリングするとき、the Dispatcher shall ホスト実行器ハンドルへ非同期タスクとして登録し busy-wait を禁止する。
2. If Mailbox の poll_once が NeedReschedule を返す場合、then the Dispatcher shall ホスト実行器の waker/notify を用いて同じ Mailbox の再実行を要求する。
3. While Mailbox が Pending 状態で新規メッセージ到着を待っている間、the Mailbox shall ホスト実行器互換の waker インターフェースを公開しスピンループなしで待機する。
4. When ホスト実行器が利用不能または過負荷を検出するとき、the Dispatcher shall Mailbox を内部キューへ退避し EventStream へ backpressure 指標を発行する。
5. The no_std Runtime shall 同一の Mailbox/Dispatcher 契約を維持しつつホスト実行器ブリッジをバイパスする。
