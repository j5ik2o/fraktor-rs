# Gap Analysis: pekko-compatible-scheduler

## 1. 現状調査
### 関連資産と構造
- `modules/utils-core/src/timing/dead_line_timer/*`: DeadlineTimer 抽象 (`DeadLineTimerTrait`, `TimerDeadLine`, `DeadLineTimerKeyAllocator`) があるが、今後削除予定であり Scheduler 基盤では利用しない。代替として `MonotonicClock` + `TimerWheelFamily` を `utils-core` に新設する計画が必要。
- `modules/utils-core/src/timing/delay/*`: `DelayProvider` と `ManualDelayProvider` が存在し、`DelayFuture` で一定時間後に完了する future を生成できる。Mailbox や Queue のタイムアウトで使用されている。
- `modules/utils-core/src/runtime_toolbox.rs`: Toolbox は Mutex ファミリのみを提供し、タイマー/クロック抽象が未定義。
- `modules/actor-core/src/dispatcher/*` および `mailbox/*`: Dispatch や Mailbox で schedule/wake の語は出るが、これは executor キューであり、タイマーとは無関係。

### 既存規約・依存方向
- steering どおり `utils-core → actor-core → actor-std` の依存構造を厳守。タイマー実装も `utils-core` で no_std 前提の Primitive を定義し、`actor-core` で型パラメータ TB (`RuntimeToolbox`) によって切り替えるパターンを踏襲する必要がある。
- 1ファイル1型／tests.rs 分離、rustdoc=英語・その他=日本語のコメント規約あり。

### 既存統合面
- Mailbox/Queue のタイムアウトは `DelayProvider` に依存しており、Scheduler も同様の provider 抽象を利用するか拡張するのが自然。
- EventStream, DeadLetter, system mailbox などの観測・監督面は `actor-core` 内で確立済み。Scheduler からのメッセージ配送は system mailbox 経由に合わせる必要がある。

## 2. 要件満足性とギャップ
| 要件 (R#) | 技術ニーズ | 既存資産 | 状態 |
| --- | --- | --- | --- |
| R1 ドリフト/上限/エラー | タイマー精度管理、maxFrequency 公開、IllegalArgumentException 相当 | 既存タイマー Primitive は期限保持のみで、公開 API・ドリフト制御が無い | **Missing** |
| R2 周期・fixed-rate | 固定レート補償、GC バースト通知、保留上限 | 該当機能なし | **Missing** |
| R3 Toolbox 連携 / TaskRunOnClose | Toolbox 拡張（タイマー/クロック）、ActorSystem lifecycle hook | Toolbox には Mutex のみ。shutdown hook は ActorSystem 内に存在するが Scheduler 用ではない | **Missing** |
| R4 並行安全性 & Cancellable | thread-safe schedule/cancel, EventStream 警告、メトリクス | 共通 `Cancellable` 実装なし。EventStream 発火ユーティリティは存在 | **Partial (警告だけは流用可)** |
| R5 テスト/診断 | 仮想クロック、決定論モード、ダンプ | `ManualDelayProvider` で擬似クロックはあるが Scheduler 専用 API なし | **Missing** |

### 不明点 / Research Needed
- 新設する `TimerWheelFamily` の tick 解像度・容量設計（no_std でのメモリ制約下での性能・空間効率）。
- RuntimeToolbox へタイマー機能を追加する際の API 形状（trait object or associated type）と、各ターゲット（no_std vs std）での具体実装。
- TaskRunOnClose 等の Shutdown hook をどの層（actor-core vs actor-std）で実装するか。

## 3. 実装アプローチ案
### Option A: DelayProvider ベースの拡張
- 既存 `DelayProvider` / `DelayFuture` を拡張し、Scheduler は DelayFuture を連鎖させて周期実行を構成する。
- Pros: 既存 API を薄く拡張するのみで済み、Mailbox タイムアウトなどとの整合が容易。
- Cons: DelayFuture は単発 Future 前提のため、固定レート補償や大規模タイマー管理に不向き。大量ジョブでの効率や cancel 追跡が困難。

### Option B: 新規 Scheduler サブシステムを構築
- `actor-core/src/scheduler/` ディレクトリを新設し、`SchedulerService`, `CancellableHandle`, `TimerCommand` などを定義。`utils-core` には `TimerWheel` + `MonotonicClock` 抽象を追加。
- Pros: 責務分離が明確で、Pekko API (`scheduleOnce`, `scheduleAtFixedRate`, etc.) を Rust 流に設計しやすい。既存 DelayProvider には影響を与えない。
- Cons: 大量の新規コード。RuntimeToolbox 拡張やテストハーネスも一から作る必要がある。

- `utils-core` へ新規 `time/` モジュールを追加し、`MonotonicClock` トレイトと `TimerWheelFamily` を提供。`RuntimeToolbox` に `type Clock` / `type Timer` を追加して各ターゲットの実装をぶら下げ、`actor-core` 側は `SchedulerService` facade を新設する。
- Pros: `RuntimeToolbox` を経由してクロックとタイマーを差し替えられるため、no_std / std で一貫性を保てる。DeadlineTimer 廃止方針とも矛盾しない。
- Cons: Toolbox 拡張・TimerWheel 実装・ハンドル管理をすべて新規に作る必要があり、設計ボリュームが大きい。

## 4. 複雑度とリスク
- **Effort**: L (1–2 週間)。理由: RuntimeToolbox 拡張、Scheduler API 設計、Timer 実装、system mailbox 連携、計測/診断まで広範囲。
- **Risk**: Medium-High。理由: no_std での正確なタイマー実装、Pekko 固有セマンティクス（固定レート補償、TaskRunOnClose など）を再現する必要があり、スケジューラの精度や負荷が不透明。

## 5. デザインフェーズへの推奨事項
- **推奨アプローチ**: Option C（RuntimeToolbox に `MonotonicClock` / `TimerWheelFamily` をぶら下げ、新規 Scheduler facade を構築）。DeadlineTimer 依存を断ちつつ、ActorSystem 連携を明確にできる。
- **重点設計ポイント**:
  - RuntimeToolbox 拡張: `type Clock: MonotonicClock` と `type Timer: TimerWheelFamily` を追加し、no_std 実装（例: SysTick/DWT）と std 実装（`Instant::now() + TimerWheel`）を差し替え可能にする。
  - `time/` モジュールで `TimerWheelConfig`, `TimerEntryMode`（ワンショット/固定レート/固定遅延）を設計し、Scheduler が固定レート補償や TaskRunOnClose を扱えるようにする。
  - `SchedulerService` の内部ステート（timer wheel, command queue, cancellable registry）の型設計と ActorSystem 連携。
  - EventStream／diagnostic hook とメトリクス収集の責務分担。
- **Research Needed**:
  1. no_std で動作する Timer Wheel 実装（tick サイズ、最大遅延）に関する最適化手法。
  2. Pekko `AtomicCancellable` のような lock-free Cancellable 実装を Rust で再現するパターン。
  3. TaskRunOnClose を ActorSystem shutdown シーケンスに組み込むための現行コードパス（`system/system_state.rs` 等）の詳細確認。
