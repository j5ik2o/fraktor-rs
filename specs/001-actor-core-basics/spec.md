# 機能仕様: Protoactor互換 Actor Core 基盤

**ブランチ**: `001-actor-core-basics`  
**作成日**: 2025-10-28  
**ステータス**: Draft  
**入力**: ユーザ要望: "protoactor-goの基本機能を真似的当該プロジェクトでも基本機能します。仕様を作成してください。一気に書き出すと範囲が広いので、actor-coreのところだけ作成してください。"

> 原則3遵守のため、protoactor-go の `actor` パッケージ（例: `actor/root_context.go`, `actor/pid.go`, `actor/supervision.go`）を参照し、Rust 版 actor-core へ落とし込む際は同等の概念・命名と逸脱理由を明示する。

## ユーザーストーリーとテスト（必須）

### ユーザーストーリー1 - アクター生成とメッセージ送信（優先度: P1）

セルアクター利用開発者として、protoactor-go と同様の API 概念でアクターを生成し、RootContext 経由でメッセージを送信できるようにしたい。これにより既存の protoactor-go サンプルをほぼ写経する形で Rust へ移植できる。  
参照: protoactor-go `actor/root_context.go`, `actor/props.go`

**優先度の理由**: アクター生成とメッセージ配送が成立しなければ actor-core の価値が生まれないため。  
**独立テスト**: 組込みターゲットとホスト双方で RootContext からエコーアクターを spawn → `Tell` → 応答を Future で受け取る結合テスト。  

**受け入れシナリオ**:

1. **前提** Rust 版 Props にメッセージハンドラを登録済み、RootContext が起動済み。**操作** `root.spawn(props)` を呼び出し、戻り値の PID へ `Tell` でメッセージを送る。**結果** ハンドラが 1 回だけ実行され、RootContext 側で完了が観測できる。  
2. **前提** 1 と同じ。**操作** `RequestFuture` によりタイムアウト付きでメッセージを送信する。**結果** 応答がタイムアウト以内に Future に格納され、エラーにならない。

---

### ユーザーストーリー2 - メールボックスとスケジューリング制御（優先度: P2）

セルアクター利用開発者として、protoactor-go が提供する bounded/unbounded mailbox やデフォルトディスパッチャと同等の制御を Rust 版で利用したい。これにより負荷制御や backpressure の挙動を揃えられる。  
参照: protoactor-go `mailbox/bounded_mailbox.go`, `actor/dispatcher.go`

**優先度の理由**: メールボックス仕様がずれると並行性特性が失われ、protoactor 互換性を名乗れなくなるため。  
**独立テスト**: Bounded mailbox に閾値 +1 のメッセージを投入して backpressure が発生する統合テスト、およびディスパッチャがスケジュール順序を保持するプロパティテスト。  

**受け入れシナリオ**:

1. **前提** 容量 10 の bounded mailbox を設定した Props。**操作** 11 件のメッセージを連続送信。**結果** 11 件目が保留またはエラーとして通知され、既存 10 件のハンドリングは順序通り完了する。  
2. **前提** デフォルトディスパッチャを使用する Props。**操作** 3 つの PID へ Round-Robin でメッセージを送信。**結果** スケジューラが protoactor-go と同じ順序で処理を割り当てることがログで確認できる。

---

### ユーザーストーリー3 - 監視とエラー回復（優先度: P3）

セルアクター運用者として、protoactor-go の supervision 戦略を Rust 版で再現し、アクター崩壊時に自動再起動や停止が選択できるようにしたい。  
参照: protoactor-go `actor/supervision.go`, `actor/restart_statistics.go`

**優先度の理由**: 基本メッセージ処理より優先度は落ちるが、安定運用には必須であり protoactor-go 互換性を担保する要素。  
**独立テスト**: 監視下の子アクターに失敗を発生させ、OneForOne/Restart 永続回数の動作を確認するシナリオテスト。  

**受け入れシナリオ**:

1. **前提** RootContext が OneForOne + Restart(最大3回, 60秒窓) を設定。**操作** 子アクターに失敗を誘発するメッセージを送信。**結果** 子アクターが 3 回まで再起動し、4 回目で停止する。  
2. **前提** スーパーバイザが Stop 戦略を設定。**操作** 子アクターがパニック相当のエラーを返す。**結果** 子アクターが停止し、監視側へ停止イベントが通知される。

### 境界条件・例外

- Bounded mailbox の容量を 0 に設定した場合は設定エラーとして扱い、Spawn 前に失敗を返す（参照: protoactor-go `mailbox/config.go`）。  
- PID が無効（終了済み・未登録）な場合、RootContext はエラーを Result 型で返し、呼び出し側が再送 or 破棄を決定できる。  
- no_std ターゲットで時間ベースのタイムアウトを提供できない場合は、utils-core の時間抽象を利用したポーリングフォールバックを必須とする。  
- 監視戦略が設定されていない場合、デフォルトで Restart 0 回・Stop 通知を採用し、予期せぬ再起動を避ける。  

## 要件（必須）

### 機能要件

- **FR-001**: システムは RootContext と Props を用いてアクターを生成し、PID を返却しなければならない。Protoactor-go `actor/root_context.go` と同じ設定項目（スーパーバイザ、メールボックス、ミドルウェア）を表現できること。  
- **FR-002**: システムは PID を通じて非同期メッセージ `Tell`, 応答待ち `RequestFuture`, 双方向通信 `Request` を提供し、メッセージ Envelope に送信者 PID とヘッダー情報を保持しなければならない。参照: protoactor-go `actor/pid.go`, `actor/message_envelope.go`。  
- **FR-003**: システムは Inbound/Outbound ミドルウェアチェーンを構成し、メッセージ処理の前後で共通処理を注入できるようにしなければならない。適用順序は protoactor-go `actor/middleware.go` と一致させる。  
- **FR-004**: システムは bounded/unbounded mailboxes とスケジューラフックを提供し、容量超過・遅延時の backpressure を明示的に通知しなければならない。参照: protoactor-go `mailbox/bounded_mailbox.go`, `actor/dispatcher.go`。  
- **FR-005**: コア機能は `#![no_std]` 環境で動作し、`std` 依存は `cfg(test)` または別クレートに限定しなければならない。共有参照・ロック機構は必ず `modules/utils-core` の `Shared`/`ArcShared` および `AsyncMutexLike`/`SyncMutexLike` 抽象を用い、`alloc::sync::Arc` やプラットフォーム固有 Mutex への直接依存を禁止する。`tokio` や `embassy` などのランタイム依存は `modules/*-std` または `modules/*-embedded` に隔離し、`modules/*-core` では利用しない。  
- **FR-006**: システムは OneForOne/AllForOne 監視戦略、Restart/Stop/Resume アクション、再起動回数・時間窓の構成を提供し、protoactor-go `actor/supervision.go` と同じ条件で再起動・停止を判断しなければならない。  
- **FR-007**: システムは PID 監視機能（Watch/Unwatch）と停止通知を提供し、protoactor-go `actor/watch.go` に沿って親子のライフサイクルイベントを通知しなければならない。  
- **FR-008**: 新規コードは actor-core 既存モジュールおよび protoactor-go 参照実装の設計パターン（トレイトオブジェクトの利用、Props ビルダの命名、PID 表現）と整合しなければならない。意図的に乖離する場合は spec/plan/tasks に根拠と参照ファイルを記録する。  

### 重要エンティティ

- **ActorSystem**: ルートコンテキストとディスパッチャを管理し、アクター登録・停止を司る。主要属性は ProcessRegistry 参照、デフォルト Dispatcher、システム用 PID。  
- **Props**: アクター生成時の設定を保持するビルダ。ハンドラ、メールボックス設定、スーパーバイザ、ミドルウェアを保持し、Spawn 時に不変となる。  
- **PID**: プロセス識別子。ProcessRegistry 内のエントリを指し、リモート識別子拡張を見据えて ID とアドレスを保持する。  
- **MessageEnvelope**: 送信者 PID、ヘッダー、メッセージ本体、レスポンスチャネルをまとめるコンテナ。  
- **SupervisorStrategy**: 監視方針を表し、失敗時のアクション・再起動統計・時間窓を保持する値オブジェクト。  

## 成功指標（必須）

### 定量的成果

- **SC-001**: サンプルアクター（単純な計算アクター）を用いたベンチで、RootContext から `Tell` したメッセージの 95% が 5ms 以内に処理完了となること（組込み・ホスト双方の標準計測環境）。  
- **SC-002**: Bounded mailbox において、設定容量を超えたメッセージ投入時に 100% のケースでエラー通知または保留制御が行われ、メッセージ消失が発生しないこと。  
- **SC-003**: 監視戦略テストで 3 回連続失敗時の再起動/停止動作が期待通りに遷移し、意図しない再起動が 0 件であること。  
- **SC-004**: Protoactor-go のチュートリアルサンプル 5 本を写経した検証コードにおいて、API 差分による修正箇所が 10% 以下（行数比）に収まり、移植作業が 1 人日以内で完了したと評価されること。  

## 前提・制約

- 参照元は protoactor-go v1 系の `actor` パッケージとする。新機能（クラスタリング、リモート）は本スコープ外。  
- actor-core 内でジェネリックとトレイトオブジェクトが混在する場合は、protoactor-go の慣例に従ってトレイトオブジェクト優先とする。  
- no_std 環境での時間管理は utils-core のタイマー抽象に依存し、actor-core では新たなタイマー機能を持たない。  
- 例外的に host 専用機能（ログの標準出力など）が必要な場合は `actor-std` への拡張タスクとして扱い、本仕様の達成条件には含めない。  

## 非目標

- クラスター間メッセージング、リモート PID 解決、分散 Pub/Sub など protoactor-go の高度機能。  
- gRPC や HTTP など外部 I/O との直接統合。  
- メトリクス収集・トレース出力など observability 拡張。  
