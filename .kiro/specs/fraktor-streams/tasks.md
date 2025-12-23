# 実装計画

- [x] 1. コアのストリーム構成要素を定義する
- [x] 1.1 Source/Flow/Sink の最小形状と接続可能性を整える
  - 入出力形状を表す抽象を用意し、組み合わせ可能な型安全境界を作る
  - StreamGraph へ登録可能な構成要素として扱えるようにする
  - _Requirements: 1.1_

- [x] 1.2 StreamGraph の接続管理と型不一致の拒否を整える
  - 接続関係を保持し、無効な接続を拒否できるようにする
  - 合成済み RunnableGraph を生成できるようにする
  - _Requirements: 1.2, 1.3, 2.1_

- [ ] 1.3 Source/Flow/Sink の DSL コンビネータを追加する
  - Pekko Streams 準拠の基本コンビネータを提供する
  - `via`/`to` の合成規則（MatCombine）を維持する
  - `map`/`flatMapConcat` を提供する
  - Source の `single` を提供する
  - Sink の `ignore`/`fold`/`head`/`last`/`foreach` を提供する
  - Source 合成は Source を返し、Sink 合成は Sink を返す形状維持を保証する
  - Source/Flow の共通コンビネータは同一シグネチャで揃える
  - DSL 経由で StreamGraph を構成できるようにする
  - _Requirements: 1.4_

- [ ] 1.4 GraphStage の中核抽象を整備する
  - GraphStage/StageLogic の最小インターフェイスを整備する
  - 組み込みステージを GraphStage として表現できるようにする
  - GraphStage を traversal に登録できるようにする
  - MatCombine に従って GraphStage の Mat を合成できるようにする
  - 公開 API に actor 型が露出しないことを確認する
  - _Requirements: 1.5, 6.7_

- [ ] 1.5 GraphInterpreter の実行契約を整備する
  - StageLogic の呼び出し順序（pull/push/complete/error）を定義する
  - StreamHandle::drive と DemandTracker/StreamBuffer の統合を定義する
  - 完了/失敗/キャンセルの伝播規則を明文化する
  - _Requirements: 1.5, 3.1, 3.2, 3.3, 4.1, 4.2, 4.3, 5.1, 5.2, 5.3_

- [x] 2. マテリアライズ値の合成と実行開始の契約を整える
- [x] 2.1 MatCombine と Materialized の合成規則を定義する
  - 合成規則が一貫して適用されることを保証する
  - マテリアライズ値の組み合わせ規則を固定する
  - _Requirements: 2.2, 2.3_

- [x] 2.2 Materializer の起動・停止と実行状態の規約を整える
  - 実行開始/停止のライフサイクルを明確にする
  - StreamHandle が実行状態を管理できるようにする
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 2.3 Materializer の拡張性を確保する
  - ActorMaterializer 以外の実装を追加できる設計であることを明文化する
  - _Requirements: 3.4_

- [ ] 2.4 StreamCompletion の最小 API と std 変換を整備する
  - core 側で `poll`/`try_take` を持つ StreamCompletion を定義する
  - std 側で actor Future への変換アダプタを用意する
  - 公開 API に actor 型が露出しないことを確認する
  - _Requirements: 2.2, 2.3, 5.1, 5.2, 5.3, 6.7_

- [x] 3. 需要伝播とバックプレッシャをコアに実装する
- [x] 3.1 DemandTracker の需要伝播と request(0) 拒否を整える
  - demand の合算と上限到達時の Unbounded 取り扱いを決める
  - 需要が無い場合に生成を抑止できるようにする
  - _Requirements: 4.1, 4.2_

- [x] 3.2 StreamBuffer のバッファ制御と容量方針を整える
  - 共通キュー実装を使ってバックプレッシャを制御する
  - 過剰なバッファ消費を抑制する
  - _Requirements: 4.3_

- [x] 4. 完了・失敗・キャンセルの伝播を実装する
- [x] 4.1 完了通知と失敗通知の伝播を整える
  - 正常完了と失敗の伝播ルールを定める
  - 完了状態が下流で観測可能になるようにする
  - _Requirements: 5.1, 5.2_

- [x] 4.2 キャンセル伝播と状態遷移の整合を確認する
  - キャンセルが上流へ伝播することを保証する
  - 状態遷移が単方向であることを確認する
  - _Requirements: 5.3_

- [x] 5. no_std/std 境界を保ちつつ core を完成させる
- [x] 5.1 core 側の no_std ビルド互換を確保する
  - std 依存を持ち込まない構成でビルドできることを確認する
  - core 公開 API が no_std で利用可能であることを確認する
  - _Requirements: 6.1, 6.3_

- [x] 5.2 std 拡張の境界を定義し、依存を隔離する
  - std 実装が core に依存する一方向の境界を維持する
  - std 無効時に依存が要求されない状態を作る
  - _Requirements: 6.2, 7.3_

- [ ] 5.3 core で fraktor-actor の実行基盤を再利用する
  - fraktor-actor std への依存を避け、core では fraktor-actor core の Scheduler/TickDriver/Extension を利用する
  - fraktor-actor 依存は必要最小限に抑える
  - _Requirements: 6.4, 6.5_

- [ ] 5.4 actor/core 依存方向と API 境界を検証する
  - actor/core から streams/core への依存が発生していないことを確認する
  - streams 公開 API に fraktor-actor の型が露出しないことを確認する
  - _Requirements: 6.6, 6.7_

- [ ] 6. Actor 実行基盤との統合を整える
- [ ] 6.1 ActorMaterializer で実行を駆動できるようにする
  - ActorSystem と統合して実行を開始/停止する
  - Materializer を通じた ActorSystem 利用を確認する
  - ActorSystem 未提供時に起動を失敗させる
  - _Requirements: 7.1, 7.2, 7.4_

- [ ] 6.2 StreamDriveActor で drive を周期実行する
  - StreamHandle を登録/解除できるようにする
  - ActorSystem のスケジューラで drive tick を行う
  - _Requirements: 7.1, 7.2_

- [ ] 6.3 TokioMaterializer と tokio 依存を整理する
  - ActorSystem 統合へ一本化し、TokioMaterializer を削除/置換する
  - examples/feature から tokio 前提を外す
  - _Requirements: 7.1, 7.2, 7.3_

- [ ] 6.4 remote/cluster 環境での利用前提を満たす
  - ActorSystem の remote/cluster 有効時でも起動条件が変わらないことを確認する
  - Materializer/DriveActor が remote/cluster の状態に依存しないことを確認する
  - _Requirements: 7.5_

- [x] 7. テストで最小構成の動作を確認する
- [x] 7.1 core の型不一致拒否と demand 制御の単体テストを追加する
  - 型不一致の拒否が観測できることを確認する
  - demand の抑止が機能することを確認する
  - _Requirements: 1.2, 4.2_

- [x] 7.2 完了/失敗/キャンセルの伝播テストを追加する
  - 完了と失敗が下流に到達することを確認する
  - キャンセルが上流へ伝播することを確認する
  - _Requirements: 5.1, 5.2, 5.3_

- [ ] 7.3 std 統合と no_std ビルドの検証を追加する
  - ActorMaterializer の起動/停止を検証する
  - no_std/std の両方でビルドが通ることを確認する
  - _Requirements: 6.1, 6.2, 6.3, 7.1, 7.2, 7.3, 7.4_

- [ ] 7.4 remote/cluster 有効時の起動スモークを追加する
  - cluster/remote の構成を有効化しても Materializer が起動できることを確認する
  - _Requirements: 7.5_

- [ ] 8. examples を通じた最小利用例を用意する
- [ ] 8.1 ActorSystem を利用した最小ストリームサンプルを追加する
  - Source/Flow/Sink と Materializer の最小合成を示す
  - ActorSystem 実行基盤で動作することを示す
  - core に std 依存を持ち込まない
  - DSL を利用した最小構成で提供する
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_
