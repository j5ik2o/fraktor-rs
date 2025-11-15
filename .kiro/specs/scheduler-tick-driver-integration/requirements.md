# Requirements Document

## Introduction
本仕様は、std（Tokio など）と no_std（embassy/SysTick など）で共通に利用できる SchedulerContext の tick 供給経路を Driver 抽象へ統一し、Runner API をテスト用途に限定することで、ActorSystem 起動直後から決定論的な tick ストリームを保証することを目的とする。

## Requirements

### Requirement 1: std 自動 Tick 供給
**Objective:** As a ランタイム利用者, I want ActorSystem 起動時に tick が自動供給されてほしい, so that std 環境で手動ポーリングなしにアクターを実行できる。

#### Acceptance Criteria
1. When ActorSystem が Tokio などの std 実行環境で `SchedulerContext` を初期化し tick driver 構成が自動モードに設定されているとき, the Scheduler Tick Driver shall 即座にホストタイマへバックグラウンドタスクを登録し、`SchedulerTickHandle` へ手動介入なしで tick を供給する。
2. While 自動 tick driver が std 実行環境で稼働している間, the Scheduler Tick Driver shall 構成済み tick 間隔の ±5% 以内の周期で連続的に tick を配送する。
3. If Scheduler Tick Driver がホストタイマ API の登録に失敗した場合, then the ActorSystem shall 起動を中止して `SchedulerContext` 初期化エラーを発火する。
4. Where ホストランタイムが Tokio マルチスレッド実行器を提供している場合, the Scheduler Tick Driver shall 専用タスクを spawn して actor workload と tick 供給を分離する。
5. The Scheduler Tick Driver shall EventStream へ直近 1 秒あたりの tick 数メトリクスを発行する。

### Requirement 2: no_std ドライバ抽象化
**Objective:** As a 組込み開発者, I want 任意のハードウェアドライバを差し替えたい, so that no_std 環境でも一貫した tick 制御を維持できる。

#### Acceptance Criteria
1. When ActorSystem が no_std ターゲットでビルドされるとき, the Scheduler Tick Driver shall `SchedulerTickHandle` と外部ドライバ実装を受け取る構成 API を公開する。
2. While ハードウェアタイマドライバが Scheduler Tick Driver に接続されている間, the Driver shall 受信順序を保持したまま `SchedulerTickHandle` へ tick を橋渡しする。
3. If 外部ドライバが tick 供給を停止した場合, then the Scheduler Tick Driver shall 停止イベントを `SchedulerContext` へ通知しフォールバックポリシーを起動する。
4. The Scheduler Tick Driver shall ドライバ trait を介して embassy, SysTick などのハードウェアドライバを差し替え可能にする。
5. Where テスト環境でビルドされている場合, the Scheduler Tick Driver shall テスト専用の手動ドライバ実装を許可する。

### Requirement 3: Runner API のテスト限定化
**Objective:** As a プロダクトオーナー, I want Runner API をテスト専用に留めたい, so that 本番利用者が誤って手動 tick 経路を選ばないようにできる。

#### Acceptance Criteria
1. When ActorSystem が プロダクション構成で起動するとき, the Scheduler Tick Driver shall 登録済みの自動ドライバ（Tokio、embassy、SysTick等）のみを受理し Runner API を無効化する。
2. If アプリケーションが Runner API をプロダクション設定で呼び出した場合, then the ActorSystem shall 起動を拒否して構成エラーを報告する。
3. While Runner API が テストプロファイル（`#[cfg(test)]` または明示的なテストフラグ）で使用されている間, the API shall `SchedulerTickHandle` へ手動 tick 注入とシミュレーション時間制御のフックのみを提供する。
4. When 新しい自動ドライバが Toolbox へ登録されたとき, the Scheduler Tick Driver shall Runner API ではなく自動ドライバ経由の tick 経路をデフォルトに設定する。
5. While Runner API テストプロファイルがアクティブな間, the Scheduler Tick Driver shall 自動 tick タスクの生成を抑止して手動注入経路のみを許可する。
6. The Scheduler Tick Driver shall ランタイム設定メタデータに現在有効な tick 供給手段（自動ドライバ名またはテスト手動モード）を記録する。

### Requirement 4: Quickstart & Driver 設定ガイド
**Objective:** As a ライブラリユーザ, I want 公式 Quickstart だけでタイマードライバの設定を完了したい, so that main 関数を低レベル API で埋め尽くすことなく ActorSystem を起動できる。

#### Acceptance Criteria
1. When Quickstart ドキュメントが std（Tokio 等）ターゲットを案内するとき, the document shall `ActorSystem::builder().with_scheduler_tick_driver(TickDriverConfig::auto().with_resolution(...))` 形式のサンプルコードを提示し、`main` 関数内のタイマー配線が 20 行未満で完結することを示す。
2. While Quickstart ドキュメントが no_std（embassy/SysTick）ターゲットを扱うとき, the document shall `SchedulerTickDriver::attach(handle, EmbassySysTick::new(...))` のような手動登録手順を段階的に説明し、ハードウェアタイマ抽象の境界を明示する。
3. If Quickstart が テスト/シミュレーションパスを紹介する場合, then it shall Runner API を `#[cfg(test)]` 付きの manual driver 例として別枠で掲載し、プロダクション構成とは併記しない。
4. The Quickstart ガイド shall Driver 選択マトリクス（`auto-std`, `embassy-systick`, `manual-test` 等）を 1 画面で比較できる表として盛り込み、ActorSystem へ渡す設定キー／builder メソッド名を列挙する。
5. When Quickstart が ActorSystem への設定注入方法を説明するとき, the document shall ActorSystemConfig/Toolbox Builder へのハイレベル API（例: `ActorSystemBootstrap::new().timers(driver).boot()?`）のみを記載し、`SchedulerTickHandle` など低レベルハンドルの直接操作を避けると明言する。
6. The Quickstart ガイド shall main 関数全体のテンプレート（`#[tokio::main]`, `fn main() -> !` for no_std）を含み、利用者がコピーペースト後に Tick Driver 設定箇所だけを書き換えれば済むことを保証する。
7. The Quickstart ガイドと仕様 shall ActorSystem 構築のためのビルダー層（例: `ActorSystemBuilder` または `ActorSystemBootstrap`）を前提にし、`with_tick_driver(...)` などのメソッドチェーンで設定を10〜15行以内に収められることを示す。

## Project Description (Input)

### 現状の課題
- 現状の `SchedulerContext` は no_std の決定論テストを基点に設計されており、std 版でも `SchedulerTickHandle` へ手動で tick を注入しないとタイマーが進まない。
- `std` + `tokio-executor` 環境ではユーザが「アクターを起動したのにスケジューラだけ動かない」状況に陥りやすく、サンプルも `SchedulerRunner::manual` を直接書いている。
- 将来の `embassy` / `SysTick` などハードウェアタイマ連携も考えると、tick 供給を toolbox/driver に抽象化しておくべきフェーズに来ている。

### 解決すべき要件
1. **std 環境（Tokio 等）**: ActorSystem 起動時に自動で scheduler tick が供給されること。
2. **no_std 環境**: 従来どおり任意のハードウェアドライバを注入できること。
3. **テスト環境**: Runner API はテスト専用とし、プロダクションではドライバ経由で統一された tick 流路を使うこと。

## Quickstart ガイドライン案
- **ステップ 1: 実行環境の選択** — `StdAutoTickDriver::tokio()` / `EmbassySysTickDriver::new(timer)` / `ManualTestDriver::new()` から選び、`TickDriverConfig` を構築。
- **ステップ 2: ActorSystem 構築** — `ActorSystemBootstrap::new().with_config(|cfg| cfg.timers(driver_config))` を呼び出し、main 関数内で 2〜3 行に収める。
- **ステップ 3: Tick 監視** — EventStream で `SchedulerTickMetrics` を購読するコード断片を掲載し、Quickstart 完了後すぐに周期確認ができるようにする。
- **ステップ 4: 環境別テンプレート** — `#[tokio::main] async fn main()` と `#[entry] fn main() -> !` の 2 パターンをそれぞれ 15 行以内で提示し、ドライバ差し替え箇所をコメントでマーキングする。
- **ステップ 4.5: ActorSystemBuilder の導入** — Tick driver や dispatcher、mailbox 構成を `ActorSystemBuilder::new().with_tick_driver(...).build()?` のようなメソッドチェーンで記述し、main 関数内の低レベル処理を Builder に集約する方針を解説する。
- **ステップ 5: トラブルシュート** — ドライバ登録失敗時のエラー／panic メッセージ例と再設定手順を Quickstart 下部に添付し、ActorSystem 起動前に原因を切り分けられるようにする。
