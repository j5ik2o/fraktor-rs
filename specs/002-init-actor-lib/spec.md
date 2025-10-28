# 機能仕様: Cellactor Actor Core 初期実装

**ブランチ**: `[002-init-actor-lib]`  
**作成日**: 2025-10-28  
**ステータス**: Draft  
**入力**: ユーザ要望: "docs/spec.mdを基にRust版のアクターライブラリの初期実装を作る\n\n- Untyped, Handleというキーワードが入ってるかも。制約違反かも。いずれにしても制約のほうが強いです。適切にしてほしい。\n- ActorErrorは構造体で固定になるかしら。Box<dyn ActorError>的なものがよいかしら。"

> 原則3遵守のため、protoactor-go / Apache Pekko の該当箇所調査と Rust への落とし込み方針を各節で明示すること。

## ユーザーストーリーとテスト（必須）

ユーザーストーリーは優先度付きの独立したユーザージャーニーとして記述し、単体で価値を届けられることを保証する。  
各ストーリーでは、protoactor-go と Apache Pekko で確立された振る舞いと整合しつつ、命名規約と no_std 制約に合わせた Rust での再設計方針を明記する。

### ユーザーストーリー1 - システム内で安全にアクターを起動したい（優先度: P1）

セルアクター利用開発者として、Apache Pekko Typed の `ActorSystem` / `Behaviors.setup` と protoactor-go の `RootContext` で得られる起動体験を Rust でも再現し、アクター参照がシステムスコープから漏れない安全な実行環境を得たい。

**優先度の理由**: アクターシステム起動が成立しなければ他機能を検証できず、初期リリース価値が失われるため。  
**独立テスト**: ドキュメントサンプル通りに `ActorSystem` を初期化し、スコープ内でカウンタアクターをスポーンしてメッセージを往復させる統合テストを実行。システム外へ参照を持ち出そうとした場合に拒否されることを確認する。

**受け入れシナリオ**:

1. **前提** ActorSystem がデフォルト設定で起動済み、Props による振る舞いが登録されている。**操作** システムスコープ内からアクターを生成し 1 件のメッセージを送信。**結果** メッセージが一度だけ処理され、結果がスコープ内で観測できる。
2. **前提** 上記で取得したアクター参照をスコープ外へ返却するコードパスを用意。**操作** 参照経由でメッセージ送信を試行。**結果** コンパイルもしくは実行時チェックで拒否され、ログに設計方針（スコープ外公開禁止）が記録される。

### ユーザーストーリー2 - メールボックスで負荷を制御したい（優先度: P1）

セルアクター運用者として、protoactor-go の Bounded/Unbounded mailbox と Dispatcher の挙動を Rust でも選択でき、Apache Pekko の Mailbox 設定と同等のバックプレッシャー制御を no_std 環境でも使いたい。

**優先度の理由**: メッセージ詰まりは全システムの信頼性に直結し、初期段階での安定動作を左右するため。  
**独立テスト**: 容量 10 のメールボックスに 11 件送信して保留通知を確認するテスト、および複数アクターでスケジューラ順序が維持されることを検証する負荷テストを実施。

**受け入れシナリオ**:

1. **前提** 容量 10 のメールボックスを Props で指定。**操作** 11 件のメッセージを連続送信。**結果** 11 件目が保留または明示的エラーとして報告され、先行分の順序が保持される。
2. **前提** デフォルト Dispatcher を共有する 3 つのアクターが存在。**操作** Round-Robin でメッセージを投入。**結果** ログまたはメトリクスで公平な割当が確認でき、優先度チェンジなどの逸脱が発生しない。

### ユーザーストーリー3 - 失敗時の回復方針を制御したい（優先度: P1）

セルアクター運用者として、Apache Pekko Typed の Supervision 戦略や protoactor-go の `SupervisorStrategy` で得られる再起動・停止制御を Rust で再利用し、アクター失敗時の影響範囲を短時間で限定したい。

**優先度の理由**: エラー復旧が不透明だと運用導入が不可能になり、初期パイロットの採用判断が下せないため。  
**独立テスト**: フェイルする子アクターを用意し、OneForOne/AllForOne の両戦略で再起動・停止判断が仕様通り行われることを検証するシナリオテストを実施。

**受け入れシナリオ**:

1. **前提** OneForOne + 再起動上限 3 回/60 秒の設定が有効。**操作** 子アクターに連続して回復可能エラーを発生させる。**結果** 3 回まで再起動し、4 回目で停止イベントが親に通知される。
2. **前提** Stop 戦略が設定されている。**操作** 子アクターが致命的エラー分類を返すメッセージを受信。**結果** 子アクターが停止し、監視者に原因と分類が共有される。

### 境界条件・例外

- System スコープ外からのアクター参照操作は拒否される。不可視化できないケースでは監査ログと共にフェイルファストする。
- メールボックスが許容量を超えた場合は、保留・ドロップ・抑止等のポリシーごとに通知が必須であり、暗黙のドロップは禁止する。
- メールボックスは Dispatcher からの Suspend/Resume コマンドに応答し、ユーザーメッセージ処理を停止・再開できることを保証する。
- SystemMessageQueue と UserMessageQueue を内部に持ち、システムメッセージが常に優先的に処理されるようキューを切り分ける。
- ReadyQueue への再登録フックと Throughput/Backpressure ヒント出力は常時有効であり、シグナルロス時は観測イベントとフォールバック動作（再試行または即時失敗）を定義する。DispatcherRuntime は常にスレッドプール（少なくとも 2 スレッド以上、構成可能）上で動作し、単一スレッド運用を前提とする実装を禁止する。
- DispatcherRuntime と MailboxRuntime の間には ReadyQueueCoordinator（再スケジュール調停役）と ReadyQueueLink（Mailbox 側の接続子）が存在し、再登録やスループットヒントは必ずこの経路を通す。
- ActorSystem は CoreSync 用 ExecutionRuntime をデフォルトで組み込み、Spawn 後のイベントループ制御や DispatcherRuntime 駆動を利用者へ委ねない。追加のホスト向けランタイムはオプションとして差し替え可能だが、未設定でもアクターが動作することが前提条件。
- Stash を利用するルートでは保留メッセージの順序と整合性を保証し、容量超過時に通知と代替手段（破棄不可/再試行）を提示する。
- `OverflowPolicy::Block` を選択する構成は HostAsync モードと `AsyncQueue` バックエンドを組み合わせた場合に限定し、CoreSync モードでは構成時に拒否される。
- Supervision 戦略未設定時は安全デフォルトとして停止を選択し、protoactor-go との互換性よりも Rust 実装の明瞭さを優先する。

## 要件（必須）

機能要件はテスト可能な文として記述し、`no_std` 制約や破壊的変更の扱いを明確にする。  
各要件では参照元（protoactor-go, Apache Pekko）と差分方針を併記する。  
DispatcherRuntime―ReadyQueueCoordinator―ReadyQueueLink―MailboxRuntime のスケジューリング連鎖に加え、実行基盤は ExecutionRuntime（ホスト／組込み向けランタイム）経由で差し替え可能としつつ、ActorSystem がデフォルトでランタイムを注入して利用者にイベントループを意識させない方針を徹底する。

本仕様では DispatcherRuntime―ReadyQueueCoordinator―ReadyQueueLink―MailboxRuntime から成るスケジューリング連鎖を採用し、ReadyQueueCoordinator が再スケジュール要求の唯一の窓口となる。以降の要件ではこの連鎖を前提としてバックプレッシャーや公平性の検証項目を定義する。

### 機能要件

- **FR-001**: ActorSystem はシステム内専用の実行スコープを提供し、アクター参照を外部へムーブできないようにしなければならない（Pekko `ActorSystem` のスコープ設計に倣い、protoactor-go `RootContext` との違いを仕様に残す）。
- **FR-002**: Props/Behavior ビルダは初期状態の注入・ライフサイクルフック・監視設定をチェーン設定できること（protoactor-go `Props` と Pekko `Behavior` API の共通機能を網羅し、`Handle` といった命名は使用しない）。
- **FR-003**: メールボックスはバウンデッド/アンバウンデッドを選択可能とし、オーバーフローポリシー（保留・最新破棄・最古破棄・拡張・ブロック）を構成できなければならない。`OverflowPolicy::Block` は HostAsync モード構成で `AsyncQueue` バックエンドを利用する場合にのみ有効とし、CoreSync モードでは設定段階でエラーとする。Suspend/Resume 操作を提供し、SystemMessageQueue と UserMessageQueue を分離した上でシステムメッセージを常に優先処理する。挙動は protoactor-go `mailbox` 実装と一致し、no_std 環境で利用できるメモリ戦略を明示する。
- **FR-004**: Dispatcher は公平性メトリクスを公開し、複数アクター間でのスケジュール順序が Pekko の Mailbox/Dispatcher 契約と整合することを検証可能にしなければならない。
- **FR-005**: コア機能は `#![no_std]` 環境で動作し、`std` 依存は `cfg(test)` または `modules/*-std` に隔離しなければならない。共有参照は `modules/utils-core` の抽象を利用し、直接的な `Arc` や OS 依存ロックへの依存を避ける。
- **FR-006**: ActorError 相当のエラー分類は再試行ポリシー・重篤度・時間窓を保持する拡張可能なデータモデルとして提供され、アプリケーションが列挙拡張またはトレイト実装で独自分類を追加できなければならない（protoactor-go `actor/errors.go` と Pekko `SupervisorStrategy` の分類方針を統合）。
- **FR-007**: Supervision 戦略は Restart/Stop/Resume/Escalate を備え、条件分岐を利用者が登録できる判定器として公開しなければならない。判定器の名前には `Untyped` を含めず、Rust の列挙型とクロージャで表現する方針を仕様化する。
- **FR-008**: メッセージアダプタ層は型安全な変換 API を提供し、内部での動的ディスパッチは必要最小限に留める。命名には歴史的な `UntypedEnvelope` を用いず、新しい名称（例: `ErasedMessageEnvelope`）を仕様で提示する。
- **FR-009**: イベントストリームは購読・解除・バックプレッシャーヒントを提供し、観測指標をテストで検証できる形式で公開しなければならない（protoactor-go `eventstream` と Pekko `EventStream` の差異を記述）。
- **FR-010**: 仕様で定義する公開 API はバイナリ互換ではなくソース互換を前提とし、破壊的変更を許容する代わりに変更理由と移行方針を spec/plan/tasks に記録する。
 - **FR-011**: Dispatcher と MessageInvoker は system/user 両キューからのメッセージを優先度付きで取得し、Suspend/Resume 指示と backpressure ヒントを伝搬しなければならない。protoactor-go の `dispatcher`/`mailbox`、Apache Pekko の `Dispatcher`/`MessageDispatcher` の責務分担を参考に、Rust では `MessageInvoker` トレイトとスケジューラループを仕様化する。DispatcherRuntime は N 個のワーカースレッド／タスクを前提としたスレッドプール上で動作し、シングルスレッド専用設計を禁止する。
- **FR-012**: Mailbox は Enqueue（offer）→ Signal（ready 通知）→ Dequeue（poll）の基本動線を保証し、ReadyQueue へ再登録できない場合は即時エラーまたは診断イベントを発火しなければならない。シグナルロスを防ぐため一貫したハンドシェイクを仕様に含める。
- **FR-013**: SystemMessageQueue を常に優先し、ユーザーメッセージレーンには予約枠と優先度付きエンベロープを適用する。予約枠が枯渇した場合は観測イベントで通知し、Dispatcher が処理順序を調整できるようにする。
- **FR-014**: Suspend/Resume 操作はユーザーメッセージ配送を停止・再開しつつ、システムメッセージを継続的に処理できるようにしなければならない。Suspend 状態遷移と Resume 再開時の優先順序を仕様化し、テストで検証可能にする。
- **FR-015**: オーバーフローポリシーごとの挙動（DropNewest / DropOldest / Grow / Block）を仕様化し、Block が選ばれた場合は HostAsync + AsyncQueue によるノンブロッキング待機とバックプレッシャーヒント発火を必須とする。
- **FR-016**: Mailbox と DispatcherRuntime はメトリクス（投入件数、ドロップ件数、Suspend 時間、system 予約枠使用率など）を `ObservationChannel` へ記録し、少なくとも仕様で定義するイベント種別を Quickstart とテストで検証可能にする。
- **FR-017**: ReadyQueueCoordinator との連携を通じて、処理済みメッセージ数や待機時間に基づく Throughput/Backpressure ヒントを返却しなければならない。ヒントは DispatcherRuntime がワーカープール調整に利用できる形式であること。
- **FR-018**: Mailbox はメッセージ処理前後に Middleware チェインを挿入できるようにし、前処理・後処理・トレース挿入などの拡張ポイントを提供する。チェインは no_std 互換 API で構成し、未設定時のオーバーヘッドを最小化する。
- **FR-019**: Suspend 回数と期間、system 予約枠消費量などの統計情報を集約し、利用者が監視レイヤーで参照可能な形式で提供する。計測には抽象化したクロックを利用し、no_std 環境でも一貫した値を取得できるようにする。
- **FR-020**: Stashing をサポートし、利用者が条件付きでメッセージを保留・再投入できる API を提供する。Stash 容量超過時は明示的なエラーおよび観測イベントを発火し、保留中メッセージを失わないよう再投入順序を仕様化する。
- **FR-021**: ActorSystem は ExecutionRuntime を通じて DispatcherRuntime/ReadyQueueCoordinator を駆動し、利用者が独自にイベントループやスレッドを管理しなくてもアクターが稼働することを保証する。CoreSync 用ランタイムはデフォルトで注入し、HostAsync など追加モードも `with_runtime` で差し替え可能とする。

#### Mailbox必須機能対応状況

| 機能 | 仕様上の扱い | 状態 | 参考要件・タスク | protoactor-go 参照 |
| --- | --- | --- | --- | --- |
| 基本メッセージ投入/受信 | FR-012 で offer/signal/poll のハンドシェイクと ReadyQueue 再登録を必須化 | OK（仕様化済） | FR-012, T303 | `queueMailbox` |
| System / User メッセージ優先度 | FR-003 / FR-013 で system 専用レーンと予約枠、優先度制御を要求 | OK（仕様化済） | FR-003, FR-013, T303 | `systemMailbox` |
| Suspend / Resume 制御 | 境界条件・FR-014 で明文化。Resume 後の再スケジュールも定義 | OK（仕様化済） | FR-014, T303 | `SuspendMailbox` |
| オーバーフロー処理 | FR-015 で全ポリシーと Block=HostAsync 制約を規定 | OK（仕様化済） | FR-015, T303 | `mailbox/queueMailbox` |
| メトリクス連携 | FR-016 で投入/ドロップ/Suspend 時間等の観測を要求 | PARTIAL（仕様あり・細部タスク化） | FR-016, T305, T311 | `mailbox/statistics` |
| ReadyQueue 連携 | FR-012 と FR-017 で ReadyQueueCoordinator 連携・ヒント伝搬を必須化 | OK（仕様化済） | FR-012, FR-017, T304, T312 | `dispatcher.Schedule` |
| Throughput / Backpressure ヒント | FR-017 でヒントを DispatcherRuntime が利用できる形式に規定 | PARTIAL（仕様あり・調整タスクあり） | FR-017, T304, T312 | `dispatcher.Throughput` |
| Middleware チェイン | FR-018 でチェイン API を要求し、spec Entities に定義 | GAP（実装タスク新設） | FR-018, T311 | `MailboxMiddleware` |
| サスペンション統計・観測 | FR-016 / FR-019 で Suspend 期間・回数を観測データに含める | PARTIAL（仕様あり・集計設計が課題） | FR-016, FR-019, T305 | `mailbox/statistics` |
| Stashing / 再投入制御 | FR-020 で Stash API とエラー処理を義務付け | GAP（実装タスク新設） | FR-020, T313 | `Stash` |

### 重要エンティティ（データを扱う場合）

- **ActorSystemScope**: システム初期化時に生成される実行スコープ。アクター生成、参照管理、監査ログ出力を担い、外部公開を禁止する境界。
- **BehaviorProfile**: Props/Behavior ビルダで確定する振る舞い設定。状態初期化、メールボックス種別、監視ポリシー、計測設定などを束ねる。
- **MessageQueuePolicy**: メールボックスの容量・優先度・バックプレッシャーポリシーを記述する設定。テストで検証可能な閾値と通知内容を保持する。
- **MailboxBackend**: `SyncQueue`/`AsyncQueue` を共通化するバックエンド抽象。`OverflowPolicy::Block` を扱う際に HostAsync モードと連携し、CoreSync では同期 API を提供する。
- **DispatcherRuntime**: DispatcherConfig を元にワーカースレッド／タスクを管理し、MessageInvoker へメッセージ処理ループを割り当てる実行装置。公平性メトリクスを ObservationChannel に送る。
- **MessageInvoker**: MailboxRuntime からシステム／ユーザーメッセージを取得し、`BehaviorProfile` の `next` 関数を呼び出すエグゼキュータ。Suspend/Resume とバックプレッシャーヒントの反映を担う。
- **ReadyQueueCoordinator**: DispatcherRuntime と MailboxRuntime を橋渡しし、再スケジュール要求や throughput/backpressure ヒントを調整する調停者。公平性管理の基準を保持する。
- **ReadyQueueLink**: MailboxRuntime から ReadyQueueCoordinator へ再登録・ヒント送出を行う接続子。命名規約として `Handle` を避け、シグナリング境界を明確化する。
- **ExecutionRuntime**: DispatcherRuntime/ReadyQueueCoordinator/WorkerPool を初期化・駆動する挿入ポイント。CoreSync/HostAsync などのモードを抽象化し、ActorSystem がデフォルトを注入する。
- **ExecutionRuntimeRegistry**: ActorSystem 内で利用可能な ExecutionRuntime を登録・解決するレジストリ。`with_runtime` で差し替え可能だが、未登録時は CoreSync 実装が自動的に選択される。
- **DispatchMode**: DispatcherConfig が選択する実行モード。`CoreSync`（no_std 用の同期駆動）と `HostAsync`（ホスト向け非同期駆動）を定義し、モードごとの制約を仕様に明記する。
- **MailboxMetrics**: Mailbox の投入・ドロップ・Suspend/Resume などのイベントを表現する観測データセット。ObservationChannel 経由で外部へ伝達され、組込み／ホスト双方で解析可能とする。
- **MailboxMiddlewareChain**: メッセージ処理前後にフックを挿入するチェイン。トレースやポリシー適用などの拡張ポイントを提供し、未設定時はノーオペレーションとなる。
- **StashBuffer**: 条件付きで保留したメッセージを保持するバッファ。容量超過時のエラーと再投入順序を明示し、再開時の処理整合性を保証する。
- **RecoveryPolicy**: Supervision の戦略と ActorError 分類を関連付けるデータ。再試行回数、時間窓、重篤度などのパラメータを含む。
- **ObservationChannel**: イベントストリームおよび Dispatcher メトリクスを購読するための抽象化。遅延やドロップなどの指標を外部へ伝播する。

## 成功指標（必須）

技術に依存しない測定可能な指標を設定する。検証はホスト OS と組込み向けビルド双方で行う。

### 定量的成果

- **SC-001**: 95% のメッセージが送信から 5ms 以内に処理完了し、再試行が不要である（代表的なホスト環境と組込み環境で計測）。
- **SC-002**: 容量 10 のメールボックスで 11 件目投入時に 100% のケースで通知が発生し、既存メッセージの順序破壊が 0 件である。
- **SC-003**: Supervision テストで再起動戦略が期待値どおり遷移し、許容回数を超えた場合に停止イベントが 100% 観測される。
- **SC-004**: 代表的な protoactor-go サンプル（カウンタ）を本仕様の API へ移植する作業時間が従来比 +20% 以内に収まると評価者 3 名以上から確認される。
- **SC-005**: イベントストリームの購読・解除を 1000 回繰り返しても遅延通知や幽霊イベントの発生率が 0% である。
- **SC-006**: ActorSystem を CoreSync デフォルト構成で起動した際、利用者コードが追加のイベントループやワーカースレッドを明示せずに 100 個のアクターを spawn・停止できることを統合テストで確認する（HostAsync ランタイムへの切替も 3 行以内の設定で完了する）。

## 前提・仮定

- no_std 対応のため、タイマーやスレッドプールは既存 `modules/*-core` 抽象を必須とし、新規実行基盤への依存を追加しない。
- エラー分類は列挙型ベースで提供しつつ、将来の柔軟性確保のためにトレイトを介した拡張ポイントを同時に設ける。
- 命名規約から `Untyped`・`Handle` を排除し、新名称は設計段階でレビュー対象とする。
- ActorSystem は CoreSync 用 ExecutionRuntime をデフォルト登録し、利用者が追加コードを書かなくても DispatcherRuntime/ReadyQueueCoordinator が稼働する前提で設計する。ホスト向けランタイムはプラガブルだが、省略可能であること。
