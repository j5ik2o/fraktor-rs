# 機能仕様: セルアクター no_std ランタイム初期版

**ブランチ**: `[001-add-actor-runtime]`  
**作成日**: 2025-10-29  
**ステータス**: Draft  
**入力**: ユーザ要望: "最初のスペックです。原則に従ってください。Rust(no_std)で動作可能なアクターシステムを作りたい。ActorSystem, Actor, ActorRef, Supervisor, Mailbox, MessageInvoker, Dispatcher, ActorCell, ActorFuture, Deadletter, ActorContext, EventStream, Pid, Props, Behaviorなどを搭載した初期版の実装を作りましょう。Queueはutils-coreのAsyncQueueを使って。asyn fnは必要最低限。全体を汚染させない。循環参照をさけて。実行できるサンプルコードも作って。この初期版を土台に機能拡張できるようにして"

> protoactor-go の Minimal Actor サンプルと Apache Pekko Classic の基本アクターライフサイクルを参照し、Rust の `no_std` 制約下へ転写する指針を各節で明記する。

## ユーザーストーリーとテスト（必須）

### ユーザーストーリー1 - 最小アクターを起動してメッセージを処理する（優先度: P1）

組込み向けアプリ開発者として、protoactor-go の `examples/spawn` 相当のシナリオを Rust `no_std` 環境でも再現できるように、ActorSystem と ActorRef だけでメッセージ送受信が完結し、メッセージ本体は `dyn core::any::Any` を包む `AnyMessage` によって未型付けで扱える最小構成を提供してほしい。Apache Pekko の「Quick Start」で示される `ActorRef.tell` の遷移を Rust で確認できることを期待する。

**優先度の理由**: ランタイム採用可否を判断するための最初のユーザバリューであり、他要素の前提となる。  
**独立テスト**: 提供されるサンプルコードをターゲットボード向けの `cargo` ビルド（`no_std + alloc`）で実行し、メッセージ送受信ログをシリアル出力で検証する E2E テスト。  

**受け入れシナリオ**:

1. **前提** ランタイム初期化時に ActorSystem とルート Props が登録済み、**操作** 単一アクターを spawn して `ActorRef` に `AnyMessage::new(Ping)` を送信、**結果** アクターが `Behavior::receive` 内で `downcast_ref::<Ping>()` を通じて `Ping` を取り出し、`Pong` を返信してサンプルが完走する。
2. **前提** メッセージ処理中にキューへ 32 件の要求が積まれている、**操作** Mailbox が protoactor-go の `DefaultDispatcher` 相当の順序で処理、**結果** 全メッセージが FIFO 順守で消化され、バックプレッシャー無しで完了ログが出力される。

---

### ユーザーストーリー2 - 階層的な監視と再起動ポリシーを適用する（優先度: P2）

ランタイム統合エンジニアとして、Apache Pekko のスーパービジョンツリーを参考に、アクターの失敗を捕捉し `Supervisor` が再起動戦略を適用できるようにしたい。protoactor-go の `OneForOneStrategy` をモデルとして Rust 向けに再設計されたポリシーを設定し、負荷試験時にも自動回復できる必要がある。

**優先度の理由**: 安定稼働に必須の信頼性機能であり、初期 adopters 向け PoC に含める。  
**独立テスト**: 疑似エラーを発生させるテストアクターを用いた統合テストで、Supervisor のポリシー適用回数と停止ログを検証する。  

**受け入れシナリオ**:

1. **前提** Supervisor が `Restart` ポリシーを設定済み、**操作** 子アクターがハンドラ内で意図的にパニックを発生、**結果** Supervisor がエラーを捕捉してアクターを即時再起動し、再起動カウンタが仕様で定める最大値未満でリセットされる。
2. **前提** Supervisor が `Escalate` ポリシーを設定済み、**操作** 致命的なエラーコードを返すメッセージを処理、**結果** 上位ツリーに例外が伝播し、ActorSystem が Deadletter 経由で通知を行う。

---

### ユーザーストーリー3 - 動作を観測して運用判断を下す（優先度: P3）

プラットフォーム運用担当として、protoactor-go の `EventStream` と Pekko の `DeadLetter` ログを参考に、アクター間通信の健全性をリアルタイムで観測したい。Deadletter と EventStream を Subscribe し、未配達メッセージや状態遷移を把握できる仕組みを提供することで、運用ダッシュボードに統合できるようにする。

**優先度の理由**: 運用品質を可視化し、初期導入ユーザの信頼を確保するため。  
**独立テスト**: イベント購読 API を使った結合テストで、意図的に不達メッセージを生成し Deadletter の記録内容と EventStream 通知件数を検証する。  

**受け入れシナリオ**:

1. **前提** EventStream に外部購読者が登録済み、**操作** 任意のアクターが `Behavior::become` で状態遷移、**結果** 状態遷移イベントが購読者に配送され、タイムスタンプと PID が一致して記録される。
2. **前提** 宛先不明 PID に `tell` を送信するテストが走っている、**操作** メッセージを送信、**結果** Deadletter が未配達メッセージと原因を記録し、EventStream にも転送される。

---

### 境界条件・例外

- `no_std` + `alloc` 環境で利用可能なことを前提とし、標準ライブラリ固有 API への依存は禁止する。
- すべてのアクター間メッセージは `AnyMessage` を経由して `dyn core::any::Any` として扱い、Typed アクター API は後続フェーズでレイヤー追加する方針とする。
- AsyncQueue が満杯の場合は protoactor-go の `BoundedMailbox` を参考にバックプレッシャーをかけ、送信 API は明示的な失敗結果を返す。
- Supervisor により再起動回数が上限を超えた場合、ActorSystem はアクターを停止させ Deadletter と EventStream へ必ず通知する。
- サンプルコードは PC 上のホストビルドと LLVM 目標ボード（例: RISCV）など少なくとも 2 つのターゲットでビルドできること。

## 仮定と依存関係

- `modules/utils-core` の `AsyncQueue` および `Shared` 抽象が既存実装として利用可能であり、追加の同期プリミティブを導入しない。
- メッセージ表現は `AnyMessage` 構造体と `dyn core::any::Any` のみに依存し、Typed メッセージ/アクターは拡張レイヤーが担う。
- メッセージシリアライズは初期版ではトレイト境界のみ定義し、具象実装は後続フェーズで拡張する。
- ロガー／タイマーなどの周辺機能は既存ユーティリティを再利用し、ActorSystem 側ではインターフェイスだけを提供する。

## 要件（必須）

### 機能要件

- **FR-001**: ActorSystem は 1 回の初期化呼び出しでルート Context・PID 名前空間・Supervisor ツリーを確立し、protoactor-go の RootContext と同等の spawn API を `no_std` 下で提供しなければならない。
- **FR-002**: Actor トレイトは `Behavior` の遷移メソッド（`receive`, `become`, `unbecome`）を提供し、Apache Pekko が定義する動的ビヘイビア切替フローを Rust のライフタイム制約に合わせて実行できなければならない。
- **FR-003**: ActorRef と Pid は一意識別子を保持し、同一アクターへの再解決が O(1) で完了するルックアップテーブルを ActorSystem 内に提供しなければならない。
- **FR-004**: Mailbox は `modules/utils-core::AsyncQueue` を内部キューとして用い、protoactor-go の Mailbox 処理順序と同様の FIFO 保証を仕様化しなければならない。
- **FR-005**: Dispatcher と MessageInvoker は メッセージ取得→ビヘイビア呼び出し→ポスト処理の段階を分離し、Pekko の `Dispatcher` 設計を参考に同期／非同期両モードを後日拡張可能な形でインターフェイス化しなければならない。
- **FR-006**: Supervisor は `OneForOne` と `AllForOne` の 2 種類以上の戦略を持ち、再起動回数制限・遅延・エスカレーション条件を設定できる API を提供しなければならない。
- **FR-007**: ActorCell と ActorContext は アクターのライフサイクル状態（初期化中／稼働中／停止）と親子関係を保持し、サンプルコードで状態遷移を EventStream へ通知できなければならない。
- **FR-008**: ActorFuture は 非同期返信のための簡易 Future/Promise API を提供し、`async fn` を利用せずにポーリングベースの完了検知ができるようにしなければならない。
- **FR-009**: Deadletter と EventStream は 100% の未配達メッセージと監視イベントを記録し、購読インターフェイス（Subscribe/Unsubscribe/Publish）を提供しなければならない。
- **FR-010**: Props は アクター生成時のファクトリ・Mailbox 設定・Supervisor 紐付けを宣言的に構築でき、protoactor-go の `Props` 相当 API を Rust の所有権モデルに合わせて記述しなければならない。
- **FR-011**: すべてのメッセージは `AnyMessage` にカプセル化され、`AnyMessage::new<T>(value: T)` と `downcast::<M>()` / `downcast_ref::<M>()` を通じて型情報を遅延取得できること。
- **FR-012**: メッセージ処理ロジックは未型付けメッセージ専用の ActorContext API を提供し、Typed レイヤーと混在させない設計ガイドラインを仕様に含めなければならない。
- **FR-013**: サンプルコードは `AnyMessage` を用いた Ping/Pong を実装し、未型付けハンドリングで性能指標を満たすことを確認しなければならない。
- **FR-014**: サンプルコードは Single Producer-Consumer の Ping/Pong シナリオを提供し、ビルド成果物が 64KB RAM 制約下で 1,000 メッセージを 1 秒以内に処理することをテストで確認できなければならない。
- **FR-015**: 拡張性確認のため、Dispatcher や Mailbox を差し替えるためのトレイト境界を公開し、外部クレートからカスタム実装を挿入できるよう仕様化しなければならない。
- **FR-016**: 全コンポーネントは循環参照を避ける設計指針（例: イミュータブル PID、弱参照テーブル）を文書化し、静的解析テストで検知できるようにしなければならない。

### 重要エンティティ（データを扱う場合）

- **ActorSystem**: PID レジストリ、Supervisor ツリー、イベント配信チャネルを保持する中心コンポーネント。
- **ActorRef / Pid**: ActorCell へのメッセージ送信に利用する軽量ハンドルと一意識別子。
- **Mailbox**: AsyncQueue に基づくメッセージバッファと処理ステータス。
- **Behavior**: 現在のメッセージハンドラと次の状態遷移に関する関数ポインタ群。
- **EventStream / Deadletter**: 監視イベントと未配達メッセージの集約ポイント。
- **Props**: アクター生成時の構成（ファクトリ、Supervisor、Mailbox 設定）をカプセル化する定義。

## 成功指標（必須）

### 定量的成果

- **SC-001**: 最小構成サンプルのアクター生成から初回メッセージ処理完了までの時間が 5ms 未満（ホスト環境）、20ms 未満（組込みターゲット）であること。
- **SC-002**: 1 秒あたり 1,000 件のメッセージ送信時に Mailbox のバックログ長が 10 件以下に収束し、Drop や Deadletter への過剰蓄積が発生しないこと。
- **SC-003**: Supervisor による再起動テストで 100 件の意図的エラーのうち 95% 以上が自動復旧し、system-wide 停止へ連鎖しないこと。
- **SC-004**: EventStream と Deadletter の監視により、未配達メッセージの検出率 100% をログ検証で確認できること。

### 定性的成果

- **SC-005**: 開発者インタビュー（少なくとも 3 名）で「Rust no_std 環境で protoactor-go/Pekko のパターンを再利用できる」と評価されること。
- **SC-006**: ランタイムコア API が 3 つ以上の将来拡張アイデア（例: クラスタリング、永続化）への適用可能性を設計レビューで承認されること。
