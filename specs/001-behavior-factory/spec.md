# 機能仕様: Behavior ファクトリと監視拡張

**ブランチ**: `001-behavior-factory`  
**作成日**: 2025-10-28  
**ステータス**: Draft  
**入力**: ユーザ要望: "pekko/akkaのようにBehaviorとBehaviors(ファクトリ）が使えるようにしてください。Props::new(behavior, ...);みたいな使い方でOK あと、アクターには名前を付けられるようにしてね。Props::new(behavior).with_name(name);名前がない場合はアノニマスということで。自動的に名前をつけてください。 あと、スーパービジョンの機能も必要です。protoactor-go, pekkoを参考にして。まぁアクターモデルでは必須の概念ですが。"

> 原則3遵守のため、protoactor-go の `actor/props.go`, `actor/supervision.go` と Pekko(Akka) の `akka.actor.typed.Behavior` / `Behaviors` API を参照し、Rust 版 actor-core への落とし込みで生じる差異は各節で説明する。

## ユーザーストーリーとテスト（必須）

### ユーザーストーリー1 - Behavior ベースでアクターを構築したい（優先度: P1）

セルアクター利用開発者として、Pekko/Akka Typed と同様に `Behavior<T>` と `Behaviors` ファクトリ関数でアクターの受信ロジックを宣言的に定義したい。これにより既存の Akka チュートリアルを Rust へ移植する際、概念変換の負担を最小化できる。  
参照: Pekko `Behaviors.setup`, `Behaviors.receiveMessage`

**優先度の理由**: Behavior API が無いと pekko/akka 互換のコード資産をそのまま移植できず、本件の主要目的を達成できないため。  
**独立テスト**: Behavior で作成したカウンタアクターを `Props::new(Behaviors::receive)` から起動し、`Tell` を複数回実行する統合テスト。Behavior がステートを保持し、期待値を返すことを確認する。  

**受け入れシナリオ**:

1. **前提** `Behaviors::receive` で整数メッセージを加算する Behavior を定義。**操作** `Props::new(behavior)` で PID を生成し、`Tell` で 3 回インクリメント。**結果** Behavior 内部ステートが 3 を指し、`RequestFuture` による照会で値 3 が返る。  
2. **前提** `Behaviors::setup` を用いて起動時にリソースを初期化する Behavior を定義。**操作** `RootContext.spawn(props)` を呼ぶ。**結果** 初期化クロージャが 1 度だけ実行され、後続メッセージ処理に影響しない。  

---

### ユーザーストーリー2 - アクターに分かりやすい名前を付けたい（優先度: P1）

運用担当者として、`Props::with_name("order-handler")` のようにアクターへ論理名を設定し、未指定時にはシステムがアノニマス名を自動採番して欲しい。これによりログや監視ダッシュボードで PID をたどらずに問題個所を特定できる。  
参照: protoactor-go `actor/process_registry.go`, Pekko Typed `ActorRef` の命名規則

**優先度の理由**: 名前付けができないと監視・デバッグ性が大きく低下し、プロダクション運用が困難になるため。  
**独立テスト**: Props builder で名前あり/なし両方のアクターを生成し、ProcessRegistry に期待どおりの名前で登録されていること、重複時は自動的に一意なサフィックスが付与されることを検証するテスト。  

**受け入れシナリオ**:

1. **前提** `Props::new(behavior).with_name("order-handler")`。**操作** RootContext がアクターを spawn。**結果** PID が `"order-handler"` を名前として保持し、ProcessRegistry で同名の PID が一意に管理される。  
2. **前提** 名前未指定の Props。**操作** RootContext が 3 つのアクターを順次 spawn。**結果** PID 名が `anonymous-<timestamp/sequence>` の形式で重複なく自動採番される。  

---

### ユーザーストーリー3 - Supervision を設定して障害から回復したい（優先度: P2）

セルアクター運用者として、Behaviors と組み合わせて Pekko/Protoactor 相当のスーパービジョン戦略（Restart, Stop, Resume, Escalate）を設定し、アクター失敗時の回復ポリシーを制御したい。  
参照: protoactor-go `actor/supervision.go`, Pekko `Behaviors.supervise`

**優先度の理由**: 高可用性を担保するためには必須であり、Behavior API と同時に提供しないと設計整合性が取れない。  
**独立テスト**: 監視下の Behavior が panic 相当のエラーを返すシナリオテストで、再起動回数や停止挙動が設定通りになることを検証。  

**受け入れシナリオ**:

1. **前提** `Behaviors::supervise(behavior).with_strategy(Restart { max_retries:3, within:60s })` を適用。**操作** 連続して 4 回失敗イベントを送信。**結果** 最初の 3 回は Behavior が再初期化され、4 回目で停止して親へ通知される。  
2. **前提** Stop 戦略を設定した Behavior。**操作** 子アクターで未処理例外を発生させるメッセージを送る。**結果** 子アクターが停止し、親コンテキストで停止イベントが受信される。  

### 境界条件・例外

- 名前付きアクターで重複名を指定した場合は自動的に `name-<sequence>` を付与し、一意性を保証する。ユーザーへはログ警告で重複を通知する。  
- Behavior は `no_std` 環境で動作するため、クロージャが `Send + 'static` を満たさない場合はコンパイルエラーとなる設計とする。  
- Supervision 戦略が未設定の場合は `Restart(0回)` をデフォルトとし、親へ停止通知を送る。  
- 自動命名で生成される ID は 63 文字以内とし、リモート対応を見据えて英数字とハイフンのみを使用する。  

## 要件（必須）

### 機能要件

- **FR-001**: システムは Pekko Typed に類似した `Behavior<T>` 抽象を提供し、メッセージ処理・初期化・終了フックを純粋関数として表現できなければならない。  
- **FR-002**: システムは `Behaviors` ファクトリ（`receive`, `receive_message`, `setup`, `with_stash` など）を提供し、Props 生成時にメジャーなパターンをワンライナーで記述できなければならない。参照: Pekko `Behaviors` API。  
- **FR-003**: `Props::new(behavior)` は Behavior インスタンスを必須引数として受け取り、メールボックス／ミドルウェア設定と組み合わせられるビルダ API を提供しなければならない。  
- **FR-004**: `Props::with_name(name)` を呼び出すと指定文字列で PID を登録し、未指定時は `anonymous-<sequence>` 形式でシステムが一意な名前を採番しなければならない。  
- **FR-005**: コア機能は `#![no_std]` 環境で動作し、`std` 依存は `cfg(test)` または別クレートに限定しなければならない。共有参照・ロック機構は必ず `modules/utils-core` の `Shared`/`ArcShared` および `AsyncMutexLike`/`SyncMutexLike` 抽象を用い、`alloc::sync::Arc` やプラットフォーム固有 Mutex への直接依存を禁止する。`tokio` や `embassy` などのランタイム依存は `modules/*-std` または `modules/*-embedded` に隔離し、`modules/*-core` では利用しない。  
- **FR-006**: システムは `Behaviors::supervise` あるいは同等の API で Restart/Stop/Resume/Escalate を設定できるようにし、再起動回数と時間窓を構成可能としなければならない。  
- **FR-007**: Supervision 戦略は階層的に適用され、親 Behavior が子アクターの失敗イベントを受け取り、protoactor-go `actor/supervision.go` と同等の判定ロジックで再起動または伝播を決定しなければならない。  
- **FR-008**: Behavior と Props API は actor-core 既存モジュール及び参照実装の命名・抽象パターンと整合しなければならない。意図的に乖離する場合は spec/plan/tasks に根拠と参照ファイルを記録する。  

### 重要エンティティ

- **Behavior<T>**: メッセージ型 T を処理する純粋関数。現在のステートとメッセージに基づき次の Behavior を返す（Peekko Typed の Become に相当）。  
- **Behaviors ファクトリ**: よくある初期化・受信・監視ラップを生成するヘルパ集合。`setup`, `receive`, `supervise`, `with_stash` などを含む。  
- **Props**: Behavior とオプション（名前、メールボックス設定、監視戦略）を束ねるビルダ。`with_name`, `with_mailbox`, `with_supervision` などのチェーンを提供。  
- **SupervisionStrategy**: Restart/Stop/Resume/Escalate と再起動統計を保持する値オブジェクト。  
- **ProcessRegistry**: 名前付き/匿名 PID を管理し、重複チェックと自動採番を行うレジストリ。  

## 成功指標（必須）

### 定量的成果

- **SC-001**: Behavior API で実装したカウンタサンプルの移植時間が protoactor-go 版と比較して 20% 以内の差で完了する（ヒアリング評価）。  
- **SC-002**: 名前未指定のアクターを 10,000 個連続生成した場合でも PID 名の衝突件数が 0 件であること。  
- **SC-003**: 再起動戦略テストで設定した最大リトライ回数・時間窓と実際の挙動が 100% 一致し、ログに意図通りの監視イベントが記録されること。  
- **SC-004**: Behavior ベースで実装したエコーアクターの 95% のメッセージが 5ms 以内に処理され、従来 Props ハンドラ方式と同等のスループットを維持すること。  

## 前提・制約

- 参照元は protoactor-go v1 系 `actor` パッケージと Apache Pekko 1.0 の Typed API。  
- Behavior 定義はトレイトオブジェクト (`dyn BehaviorHandler`) を基本とし、ジェネリックは内部実装に留める。  
- 自動採番で利用する乱数・時刻は utils-core の抽象に依存し、no_std ターゲットでも決定的なシーケンスを生成できるようにする。  
- 監視戦略のテレメトリ出力は actor-core ではログイベントの発火のみとし、外部メトリクス送信は将来の `actor-std` 拡張とする。  

## 非目標

- Behavior DSL のマクロ化やコード生成といったシンタックスシュガー。  
- クラスタリング・リモートへ跨る命名解決（リモートアクターの名前衝突処理は本スコープ外）。  
- Behavior ベースでのストリーム/パイプライン統合など高水準 API。  
