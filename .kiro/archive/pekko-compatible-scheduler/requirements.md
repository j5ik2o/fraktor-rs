# Requirements Document

## Introduction
Pekko の `Scheduler` 仕様（`references/pekko/actor/src/main/scala/org/apache/pekko/actor/Scheduler.scala`）を踏襲し、no_std/STD 共通の RuntimeToolbox で動作するタイマー／スケジューラ基盤を fraktor-rs に組み込む。遅延実行・周期実行・キャンセル・診断を deterministic に扱い、ActorSystem と自然に統合できることを目的とする。

## Requirements

### Requirement 1: 決定的なタイマー実行
**Objective:** ランタイムメンテナが遅延ジョブを正確に扱えるように、Scheduler へ deterministic なワンショット実行を提供する。

#### Acceptance Criteria
1. When 呼び出し元が遅延 `d` のワンショットタスクを登録した場合、Scheduler は `d ± 1 tick` の範囲でタスクを一度だけ実行しなければならない。
2. When 同一 tick で複数タスクが満期になった場合、Scheduler は登録順（FIFO）でハンドラを呼び出さなければならない。
3. While ランタイムが平常稼働している間、Scheduler は tick あたりのドリフトを設定解像度の 5% 以下に維持しなければならない。
4. If タスクが発火前に cancel 要求を受けた場合、Scheduler はハンドラを呼び出さず 1 tick 以内にリソースを解放しなければならない。
5. When 入力された遅延や周期が 0 または負、もしくは `delay / tickNanos > Int.MaxValue` に達する場合、Scheduler は `IllegalArgumentException` 等のエラーを返して登録失敗を示さなければならない。
6. The Scheduler shall `maxFrequency` などの API で最小 tick/最大周波数を公開しなければならない。

### Requirement 2: 周期ジョブと時間管理
**Objective:** サブシステム開発者がハートビート等を自動化できるように、周期タスク API を提供する。

#### Acceptance Criteria
1. When 周期 `p` のタスクが登録された場合、Scheduler は `t0 + n·p (n≥1)` の時刻でタスクを連続発火させなければならない。
2. If ハンドラ実行時間が周期 `p` を超過した場合、Scheduler はミスした tick をまとめて 1 回の実行へ折りたたみ、累積実行数をハンドラへ伝えなければならない。
3. While システム時間源が前後にジャンプしている間、Scheduler は RuntimeToolbox のモノトニックタイマーを基準に論理時間を維持しなければならない。
4. When 呼び出し元が fixed-delay モードを選択した場合、Scheduler は各実行完了から次の開始までの遅延を `p` として計測しなければならない。
5. When fixed-rate モードで長時間停止（GC 等）が発生した場合、Scheduler は休止中に溜まった実行を順次発火させる一方で、バースト実行を EventStream に警告として通知しなければならない。
6. When 周期ジョブが許容保留数 `k` を超えた場合、Scheduler はジョブをキャンセル状態に遷移させなければならない。

### Requirement 3: RuntimeToolbox / ActorSystem 連携
**Objective:** ランタイム統合者が no_std と std の双方で同じ挙動を得られるように、Scheduler を Toolbox/ActorSystem に適合させる。

#### Acceptance Criteria
1. When ActorSystemBuilder が初期化されるとき、Scheduler は RuntimeToolbox からタイマーリソースを取得し、std 専用 API に依存してはならない。
2. Where 実行環境が std 機能（壁時計等）を提供する場合、Scheduler は観測用フックを追加しても no_std ビルドに影響を与えてはならない。
3. While ActorSystem がシャットダウン中である間、Scheduler は未発火タスクをキャンセルし、Toolbox リソースが破棄される前にハンドラをフラッシュしなければならない。
4. If スケジューラがアクターへメッセージを投函する場合、Scheduler は system mailbox を経由して監督セマンティクスを維持しなければならない。
5. The Scheduler shall `schedule_once` / `schedule_at_fixed_rate` / `schedule_with_fixed_delay` API を Pekko と整合するシグネチャで提供しなければならない。
6. When Scheduler が close() され TaskRunOnClose タスクが保留されている場合、Scheduler は close 処理中にそれらを実行しなければならない。
7. If shutdown 済みの Scheduler に新規スケジュール要求が届いた場合、Scheduler は SchedulerException 等を発生させて失敗を通知しなければならない。

### Requirement 4: 並行安全性と負荷制御
**Objective:** プラットフォームエンジニアがマルチスレッド環境でも安全に利用できるように、Scheduler の同時実行保証を定義する。

#### Acceptance Criteria
1. The Scheduler shall 複数スレッドからの schedule/cancel 呼び出しでデータ競合が起きないようにしなければならない。
2. When cancel と実行が競合した場合、Scheduler はハンドラを高々 1 回だけ呼び出さなければならない。
3. The Cancellable shall 初回の `cancel()` 呼び出しで true を返し、以降の呼び出しでは false を返さなければならない。
4. The Cancellable shall `isCancelled == true` を一度でも cancel が成功した後は常に報告しなければならない。
5. While ランタイムがバックプレッシャ状態にある間、Scheduler はアクターシステム単位のタイマー上限を超えた時点でエラー/拒否を返さなければならない。
6. If タイマーホイールが飽和した場合、Scheduler は EventStream へ警告を送信し、優先度の低いタイマーから順に廃棄しなければならない。
7. The Scheduler shall 稼働中タイマー数・周期ジョブ数・ドロップ数をメトリクスとして公開しなければならない。

### Requirement 5: テスト性と診断機構
**Objective:** QA/オブザーバビリティ担当が Scheduler を検証・可視化できるように、テスト用フックとダンプ機能を提供する。

#### Acceptance Criteria
1. When バーチャルクロックモードが有効な場合、Scheduler は手動 tick 進行でリアル時間に依存せずジョブを発火させなければならない。
2. If 決定論モードがオンのとき、Scheduler はタスク ID と発火時刻をログに残し、リプレイ検証を可能にしなければならない。
3. While 診断モードが有効な間、Scheduler は schedule/fire/cancel イベントを診断ストリームへ出力しなければならない。
4. The Scheduler shall プロパティテストや fuzz により tick 単位の単調性・キャンセル保証・固定レート補償を検証できる API を提供しなければならない。
5. When ダンプ要求を受けた場合、Scheduler はホイール位置・保留タスク・周期ジョブ一覧を人が読める形式で出力しなければならない。
