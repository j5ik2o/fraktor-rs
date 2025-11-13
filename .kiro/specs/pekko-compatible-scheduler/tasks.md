# Implementation Plan

- [ ] 1. タイミング基盤とTickソースを整備する [優先度: CRITICAL]
  - 依存関係: 1.1でTimerWheel/Clockを完成させてから1.2でToolbox統合へ進む。1.*完了後にScheduler APIを着手可能とする。
  - 完了条件: TimerWheel/Clock単体テストとpropertyテストがCIでグリーンとなり、Toolboxモック経由でtick_leaseを取得できる。
  - _Requirements: R1.AC1-R1.AC3, R1.AC6, R2.AC3_

- [ ] 1.1 TimerWheelとMonotonicClockの決定性を実装する
  - TimerWheelFamilyとMonotonicClockを結合し、tick解像度と±1 tick誤差を守る進行ループを構築して同一tick内の順序保証をテスト10ケース以上で証明する。
  - Tickドリフト監視を追加し、累積偏差が解像度の5%を超えた際に`SchedulerWarning::DriftExceeded`を発火しEventStreamへ通知する。
  - 同一tickで満期になったエントリを登録順に取り出せるFIFOを実装し、10メッセージのFIFO統合テストで検証する。
  - ManualClock/StdInstant双方でモノトニック性と決定性を検証するproptestを追加し、ドリフトレポートをログ収集する。
  - _Requirements: R1.AC1, R1.AC2, R1.AC3, R2.AC3_

- [ ] 1.2 RuntimeToolboxのtickソースと容量プロファイルを導入する
  - RuntimeToolboxにtick_source/tick_lease APIを追加し、std/non-stdの両Toolboxでpending tickをLeaseとして引き出せることを統合テストで確認する。
  - SchedulerCapacityProfileからsystem_quota/overflow/task_run容量を設定し、検証失敗時にBuilderが即エラーを返すフェイルファストパスを追加する。
  - `Scheduler::max_frequency`やresolution getterで最小tick/最大周波数を公開し、APIのrustdocに具体例を追加する。
  - Tokio/SysTick実装がtick_source/tick_leaseを満たしていることをMockToolbox比較テストで保証する。
  - _Requirements: R1.AC6, R3.AC1_

- [ ] 2. Scheduler API表面とrustdocを固める [優先度: CRITICAL]
  - 依存関係: 1.*完了後に着手し、2.1→2.2→2.3の順で進める。2.4は2.1-2.3が揃った後に並列実施可能で、2.*完了がSystemMailbox統合の前提となる。
  - 完了条件: schedule_* APIがPekko互換シグネチャとrustdocを備え、自動テストで正/異常系を網羅する。
  - _Requirements: R1.AC5, R3.AC5_

- [ ] 2.1 schedule_* APIシグネチャと動作を実装する
  - schedule_once/at_fixed_rate/with_fixed_delayおよびRunnable版APIをPekkoと同じパラメータ順で公開し、Typed/Untyped双方から呼べるようにする。
  - 各APIの戻り値でCancellableを返し、成功時にhandle IDと実行モードが追跡できるようにする。
  - 既存DelayProviderからの呼び出し経路を新APIへ付け替え、旧APIとの比較テストで互換性を確認する。
  - _Requirements: R1.AC5, R3.AC5_

- [ ] 2.2 入力検証とエラーパスを実装する
  - delay<=0、負周期、Durationオーバーフロー、`delay / tickNanos > i32::MAX`など異常値を検証し`SchedulerError::InvalidDelay`/`IllegalArgument`で即座に失敗させる。
  - Backpressure/容量オーバー時の`SchedulerError::Backpressured`、shutdown後の`SchedulerError::Closed`などResultエラーを単体テストで網羅する。
  - _Requirements: R1.AC4, R1.AC5, R4.AC5_

- [ ] 2.3 Dispatcher/Typed facade解決を共通化する
  - DispatcherSenderShared/Senderの解決をContextやActorSystem defaultに委譲するfacadeを実装し、Typed/Untyped APIが単一路線を共有する。
  - Dispatcher未指定時にSystem defaultを採用する挙動をintegrationテストで確認し、Remoting/DelayProviderパスと突き合わせる。
  - _Requirements: R3.AC4, R3.AC5_

- [ ] 2.4 公開APIのrustdocと使用例を追加する
  - schedule_*、Cancellable、SchedulerBuilder公開APIに英語rustdocとExamplesセクションを追加し、Typed/Untyped両方のサンプルコードを含める。
  - rustdocビルドをCIに追加し、no_std環境でもコンパイル可能なドキュメント例を検証する。
  - _Requirements: R1.AC5, R3.AC5_

- [ ] 3. SystemMailboxブリッジとRunner制御を実装する [優先度: HIGH]
  - 依存関係: 2.*のAPI完成後に着手し、3.*完了が周期ジョブ実装の前提。
  - 完了条件: SystemMailbox経由で遅延メッセージがFIFOで配送され、RunnerがBackpressure通知とstop_and_flushシーケンスを扱える。
  - _Requirements: R1.AC1, R1.AC2, R1.AC3, R3.AC4_

- [ ] 3.1 SystemMailboxブリッジとExecutionBatch管理を実装する
  - SchedulerCommandからSystemMailboxへの橋渡しを実装し、SystemMessage優先順位を壊さず`UserMessagePriority::Delayed`を付与する。
  - enqueue直前にCancellableRegistryを参照し、キャンセル済みハンドルを破棄することでhandle一意性を保つ。
  - 統合テスト: 同一tickで10件のメッセージを登録しFIFO順でenqueueされること、cancel後はenqueueされないことを検証する。
  - BatchContext/ExecutionBatchをmessageとRunnable双方でpushし、Dropで`ack_complete`するguardをテストする。
  - _Requirements: R1.AC2, R3.AC4_

- [ ] 3.2 SchedulerRunnerとTickLeaseの駆動ループを実装する
  - RunnerMode::Manual/AsyncHost/Hardwareを実装し、TickLeaseからbacklogをチャンク取得してrun_tickへ供給する。
  - TickSourceStatusがBackpressuredになった場合にcatch-up chunkを発火し、ドリフト抑制と`SchedulerBackpressureLink`遷移を確認する。
  - RunnerLoop停止・再開で`stop_and_flush`→`TaskRunContext`の競合を防ぐ状態機械を追加し、state machineテストで覆う。
  - _Requirements: R1.AC1, R1.AC3, R3.AC3_

- [ ] 3.3 Runner統合テストで順序保証を検証する
  - ManualClockでtick進行を制御し、Runnerがcatch-upチャンクを処理してもFIFO順序とExecutionBatchのruns値が維持されることを確認する。
  - Backlog>catch_up_window時に`SchedulerWarning::DriverCatchUpExceeded`が発火しaccepting_stateが変化することをCIで検証する。
  - _Requirements: R1.AC3, R4.AC5_

- [ ] 4. 周期ジョブ制御とバックログ保護を実装する [優先度: HIGH]
  - 依存関係: SystemMailbox/Runnerが完成していること。
  - 完了条件: FixedRate/FixedDelayジョブがmissed_runsを折り畳み、backlog制御と警告通知が行える。
  - _Requirements: R2.AC1-R2.AC6_

- [ ] 4.1 FixedRate/FixedDelayコンテキストを構築する
  - FixedRateContext/FixedDelayContextを実装し、TimerWheelから受け取ったmissed_runsをExecutionBatchへ折り畳む。
  - ハンドラ実行時間が周期を超えた場合に1回の実行へまとめてrunsとmissedRunsを渡し、FixedDelayは完了時刻から次回開始までの遅延を再計測する。
  - GCや長時間停止でmissed_runsが閾値を超えたとき`SchedulerWarning::BurstFire`をEventStreamへ出す統計コードとテストを追加する。
  - FixedRate/FixedDelayそれぞれで最低5シナリオの統合テストを追加し、missed_runs折り畳みとburst警告が期待どおり動作することを検証する。
  - _Requirements: R2.AC1, R2.AC2, R2.AC4, R2.AC5_

- [ ] 4.2 バックログ上限と自動キャンセルポリシーを導入する
  - backlog_limitを越えた周期ジョブを自動キャンセルし、`SchedulerWarning::BacklogExceeded`を記録する。
  - 許容保留数kをSchedulerPolicyRegistryで設定し、Pendingジョブが上限を超えたら即キャンセルへ遷移させる。
  - backlog-limit/auto-cancelシナリオをManualClockで再現し、`CancelledByBackpressure`通知をEventStream/Diagnosticsへ流す統合テストを追加する。
  - _Requirements: R2.AC6_

- [ ] 5. ActorSystem統合とシャットダウンフローを仕上げる [優先度: HIGH]
  - 依存関係: 1-4完了後、ActorSystemBuilder/DelayProvider統合へ進む。
  - 完了条件: BuilderがSchedulerを初期化・closeし、shutdown時にTaskRunOnCloseがdeterministicに完走する。
  - _Requirements: R3.AC1-R3.AC7_

- [ ] 5.1 ActorSystemBuilderとDelayProviderの統合を完了する
  - ActorSystemBuilderがRuntimeToolboxからclock/timer/tick_sourceを取得し、std/no-std両用の構築ルートを整える。
  - SchedulerBackedDelayProviderを導入し、DelayFutureが新Schedulerを透過的に利用するよう内部実装を差し替える。
  - std環境限定の観測フック（tokio::Instant等）をactor-std層に閉じ込め、no_stdビルドへ影響しないことをintegrationテストで証明する。
  - _Requirements: R3.AC1, R3.AC2_

- [ ] 5.2 Shutdown・TaskRunOnClose・rejectフローを実装する
  - Builderが登録したTaskRunOnCloseハンドルを優先度順に実行し、Scheduler::shutdown内で未発火タスクをdrainする。
  - `stop_and_flush`完了後に残っているcommands/handlesをキャンセルまたは実行し、SystemMailboxへ新規enqueueしないことを保証する。
  - shutdown済みSchedulerへのschedule_*呼び出しに`SchedulerError::Closed`を即返し、新規登録を拒否する。
  - shutdown/TaskRunOnCloseフローの統合テストでRemoting/DelayProvider cleanupがdeterministicに完了することを検証する。
  - _Requirements: R3.AC3, R3.AC6, R3.AC7_

- [ ] 6. 並行安全性と負荷制御を強化する [優先度: HIGH]
  - 依存関係: 2-5のAPI/統合が完了していること。
  - 完了条件: Cancellable競合とBackpressure/容量制御が全エラーパスまでテストされ、メトリクスが収集できる。
  - _Requirements: R1.AC4, R4.AC1-R4.AC7_

- [ ] 6.1 CancellableRegistryとhandle状態機械を構築する
  - CancellableStateをPending→Scheduled→Executing→Completed/Cancelledで遷移させるlock-free状態機械を構築する。
  - cancel()が初回のみtrueを返し、それ以降falseになることと`is_cancelled`が永続的にtrueを返すことを保証する単体テストを追加する。
  - cancelと実行が競合した場合にハンドラが高々1回しか呼ばれないようcompare_exchangeガードを実装し、1 tick以内のリソース解放を検証する。
  - _Requirements: R1.AC4, R4.AC1, R4.AC2, R4.AC3, R4.AC4_

- [ ] 6.2 Backpressure・容量制御・メトリクスを実装する
  - SchedulerCapacityProfileに基づくsystem_quota/overflow_capacityを監視し、超過時に`SchedulerError::Backpressured`を返す状態遷移を実装する。
  - 低優先度タイマーをdropする際に`SchedulerWarning::DroppedLowPriority`や`CancelledByBackpressure`をEventStreamへ発行する。
  - active timers/periodic jobs/dropped totals/tick backlogなどのメトリクスをSchedulerMetrics経由で公開し、CIで数値の上限/下限を検証する。
  - _Requirements: R2.AC6, R4.AC5, R4.AC6, R4.AC7_

- [ ] 7. 診断・テスト性・パフォーマンスを仕上げる [優先度: MEDIUM]
  - 依存関係: 1-6完了後に取りまとめる。
  - 完了条件: ManualClock/Diagnostics/Benchmark/エラーカバレッジが全要件を検証し、CIジョブが追加される。
  - _Requirements: R5.AC1-R5.AC5, R4.AC7_

- [ ] 7.1 ManualClock・決定論ログ・プロパティテストを整備する
  - ManualClock/ManualTimerをSchedulerRunnerへ接続し、手動tickでジョブを発火できるテスト専用モードを実装する。
  - DeterministicLogにタスクID/発火時刻/実行モードを記録し、リプレイ検証APIを提供する。
  - Property/fuzzテストハーネスを追加し、tick単調性・キャンセル保証・固定レート補償を100ケース以上で自動検証する。
  - _Requirements: R5.AC1, R5.AC2, R5.AC4_

- [ ] 7.2 Diagnosticsストリームとダンプを実装する
  - DiagnosticsFanout（heapless/tokio）を介してschedule/fire/cancel/DriftWarningを配信する診断ストリームを実装する。
  - 診断購読者が0件のときEventStreamへのフォールバックを行い、オーバーフロー時は`DiagnosticsDropped`を通知して回復動作をテストする。
  - SchedulerDump APIを実装し、wheel offset・保留タスク・周期ジョブ状態を人間が読める形式で生成し、CLI/Telemetryから取得する統合テストを追加する。
  - _Requirements: R5.AC3, R5.AC5_

- [ ] 7.3 ベンチマークとパフォーマンステストを実装する
  - 1,000/10,000タイマー同時実行時のオーバーヘッドを計測し、ドリフト率が5%以内であることをベンチマークで確認する。
  - 周期ジョブの精度（missed_runs折り畳み、burst警告）を計測し、backlog上限到達時の挙動を記録するCIジョブを追加する。
  - _Requirements: R4.AC7, R5.AC4, R5.AC5_

- [ ] 7.4 エラーパス網羅テストを追加する
  - すべての公開APIでpanicを起こさずResultエラーを返すことをコンビネーションテストで検証し、SchedulerError/SchedulerWarning全バリアントのテストカバレッジを100%にする。
  - shutdown済みSchedulerやBackpressure状態でのAPI呼び出しが想定エラーを返すことをintegrationテストで確認する。
  - _Requirements: R1.AC4, R3.AC7, R4.AC5, R5.AC3_

- [ ] 7.5 no_std環境での完全動作を検証する
  - no_std+alloc構成でScheduler API一式がコンパイル・リンク可能であることをCIジョブに追加し、heapless実装経路を含む統合テストを実行する。
  - SysTick/embassy系Toolboxでtick_source/tick_leaseが動作するモックテストを追加し、DiagnosticsFanout(heapless)のドロップ挙動を検証する。
  - _Requirements: R3.AC1, R5.AC4_

- [ ] 8. リファクタリングと技術的負債の解消 [優先度: NICE_TO_HAVE]
  - 依存関係: 1-7の実装中に洗い出した改善点をIssue/ノートへ記録し、主要タスク完了後にまとめて着手する。
  - 完了条件: clippy警告0件、重複コードの削減、module境界の整理、残存TODO/unwrapの解消を確認し、負債リストをクローズする。
  - _Requirements: R1-R5（全要件の品質維持）_
