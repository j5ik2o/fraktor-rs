# 機能仕様: 非同期 Dispatcher / MessageInvoker 設計

**ブランチ**: `001-dispatcher-async`  
**作成日**: 2025-10-28  
**ステータス**: Draft  
**入力**: ユーザ要望: "Dispatcher, MessageInvokerあたりもpekko/akkaを参考してください。no_stdで骨格を設計しますが、tokio, embassayで設計破綻がないように考慮してください。おそらく主要なロジック、Mailboxも含めて、async化しないと設計破綻するかもしれません。"

> 原則3遵守のため、Pekko/Akka の `akka.dispatch`（特に `Dispatcher`/`Mailbox`/`MessageInvoker`）と protoactor-go の `mailbox`/`dispatcher` 実装を調査し、Rust actor-core へ適合させる際の差異と非同期対応方針を各節で明示する。

## ユーザーストーリーとテスト（必須）

### ユーザーストーリー1 - 非同期 Dispatcher を差し替えたい（優先度: P1）

セルアクター利用開発者として、Pekko 同様に Dispatcher をプラガブルに差し替え、組込み向け no_std 実装と tokio/embassy ベースの async 実装を用途に応じて切り替えたい。これにより同一 API で異なる実行環境に最適化できる。  
参照: Pekko `DispatcherConfigurator`、protoactor-go `DefaultDispatcher`.

**独立テスト**: no_std モック Dispatcher と tokio ランタイム連携 Dispatcher を切り替え、アクター処理順序とメッセージスループットが期待通りとなることを確認する統合テスト。  
**優先度の理由**: Dispatcher が不十分だと ActorSystem 全体が機能しないため。

**受け入れシナリオ**

1. **前提** No_std Dispatcher (同期キュー) を構成。**操作** 100 メッセージを送信。**結果** メッセージが順序通り処理され、リソース制約に沿ったスケジューリングが行われる。  
2. **前提** tokio Dispatcher を構成。**操作** 同じメッセージパターンを async で送信。**結果** Future の完了が保証され、スケジューリングが tokio のタスクモデルと矛盾しない。

### ユーザーストーリー2 - MessageInvoker で async mailbox を統一したい（優先度: P1）

ライブラリ実装者として、Mailbox を含むメッセージ処理パイプラインを async 化し、メッセージを順序保証しつつ backpressure に対応したい。これにより no_std のポーリング実装と tokio/embassy の async 実装を同じ抽象で扱える。  
参照: Pekko `MessageDispatcher`, protoactor-go `MessageInvoker`.

**独立テスト**: async メールボックスを介してメッセージを投入し、invoker が await ポイントで処理を中断・再開できることを検証する。  
**優先度の理由**: Mailbox を同期前提で組むと async 拡張が破綻するため。

**受け入れシナリオ**

1. **前提** Async Mailbox が backpressure ポリシー `DropOldest` を設定。**操作** 非同期 invoker が順次メッセージを await しながら処理。**結果** メールボックスが溢れた際に古いメッセージがドロップされ、ログとメトリクスに記録される。  
2. **前提** tokio ランタイム下で `MessageInvoker` が `async fn invoke` を提供。**操作** 1 秒あたり 10,000 通のメッセージを送信。**結果** invoker が Future を正しく完了させ、処理遅延が閾値内に収まる。

### ユーザーストーリー3 - ランタイム間で共通の契約を維持したい（優先度: P2）

セルアクター設計者として、Dispatcher/Invoker/メールボックスの組み合わせが no_std と tokio/embassy の両方で設計破綻を起こさないことを保証するため、共通の契約テストとデザインガイドラインを整備したい。  
参照: Pekko `Dispatcher` のテストスイート、protoactor-go の `scheduler` テスト。

**独立テスト**: `cfg` 切り替えで異なる Dispatcher 実装を読み込み、共通の契約テストをパスすることを CI で検証。  
**優先度の理由**: 将来的なランタイム追加時の回帰を防ぐため。

**受け入れシナリオ**

1. **前提** CI で no_std モックを使用。**操作** 契約テストを実行。**結果** すべて成功し、enqueue/dequeue/await まわりでパニックやデッドロックが起きない。  
2. **前提** tokio と embassy を対象に同じテストスイートを実行。**結果** それぞれのランタイム固有実装でも契約を満たし、不整合がログに現れない。

### 境界条件・例外

- Mailbox/Dispatcher の抽象は `#![no_std]` ベースで定義し、`tokio`/`embassy` 実装は feature で拡張する。  
- async 対応が難しい組込み環境ではポーリングループ（`Future` を使わず `MapSystemFn` 等）を fallback とする。  
- トレイトオブジェクトを必要最小限にとどめるため、Dispatcher/Invoker はジェネリック実装を優先し、動的ディスパッチが必要な箇所は性能分析を添付する。  
- Mailbox の共通オーバーフローポリシー（DropNewest 等）は既存憲章に従い async 版でも維持する。

## 要件（必須）

### 機能要件

- **FR-001**: システムは Dispatcher 抽象を提供し、no_std（同期）と tokio/embassy（async）実装を差し替え可能にしなければならない。  
- **FR-002**: Dispatcher はメッセージ処理の前後で backpressure/メトリクスフックを呼び出し、メールボックスのオーバーフローポリシーと整合しなければならない。  
- **FR-003**: MessageInvoker は async 対応を前提とし、`Future<Output = ()>` を返す `invoke` API（または等価な async 抽象）を提供しなければならない。  
- **FR-004**: Mailbox は async パイプラインに統合され、メッセージを await 可能な形で取り出せなければならない。no_std 環境ではポーリング fallback を提供する。  
- **FR-005**: Dispatcher/Invoker/Mailbox の契約テストを共通化し、ランタイムごとの差分を最小化するテストハーネスを用意しなければならない。  
- **FR-006**: トランスポート・シリアライズは抽象インターフェイスを通じて差し替え可能とし、async Dispatcher でも特定技術（protobuf 等）に固定しない。  
- **FR-007**: 共有 `Shared` 命名規約、循環参照回避、トレイトオブジェクト最小化方針を遵守し、必要な場合は性能影響を記録する。  
- **FR-008**: すべての機能は `#![no_std]` でコンパイル可能であり、`std` の依存は feature フラグで切り替え可能にしなければならない。  

### 重要エンティティ

- **Dispatcher<T>**: メッセージ処理をスケジュールし、メールボックスから取り出したイベントを async で Invoker に渡す。  
- **MessageInvoker<T>**: アクターのハンドラクロージャをラップし、必要に応じて await とエラー処理を行う。  
- **AsyncMailbox<T>**: utils-core の queue を基盤に、async でメッセージを取り出せるメールボックス。  
- **DispatcherConfig**: ランタイム選択、スレッド数、バックプレッシャーポリシー、計測フックを含む設定オブジェクト。  
- **DispatcherMetrics**: メッセージ処理時間、待機時間、ドロップ数などを記録する観測エンティティ。  

## 成功指標（必須）

### 定量的成果

- **SC-001**: tokio Dispatcher で 10,000 msg/s を処理した際、平均レイテンシが 5ms 以下、ドロップ率 0% を達成する。  
- **SC-002**: no_std Dispatcher でも 1,000 msg/s の処理でパニックやデッドロックが発生せず、CPU 利用率が想定範囲内（±10%）に収まる。  
- **SC-003**: 共通契約テストを tokio / embassy / no_std の 3 構成で実行し、エラー件数が 0 である。  
- **SC-004**: Dispatcher/Invoker 抽象の追加によりバイナリサイズおよびランタイムメモリ増加が 10% 未満に収まる。  

## 前提・制約

- Mailbox の async I/O は `Future` ベースで設計し、no_std 向けには `poll` ベースのバックアップを用意する。  
- ランタイム固有の機能（tokio のマルチスレッド scheduler 等）は拡張として扱い、コアは同期/単スレッド前提で破綻しないようにする。  
- `Shared` 命名規約・トレイトオブジェクト最小化・トランスポート抽象化など憲章の既存ルールをすべて満たす。  
- 既存メールボックス仕様（オーバーフローポリシー、metrics、suspension）を再利用し、async 化による回帰を避ける。  

## 非目標

- tokio/embassy 以外の async ランタイム（async-std 等）への公式サポート。  
- 低レベルのスレッドプール実装（将来的な最適化範囲外）。  
- メールボックス以外の全コンポーネントの完全 async 化（本仕様では Dispatcher/Invoker/メールボックスを対象とする）。  
