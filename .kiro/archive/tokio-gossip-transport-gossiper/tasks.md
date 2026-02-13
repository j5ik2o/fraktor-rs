# Implementation Plan

- [x] 1. Tokio gossip transport の送受信基盤を構築する
- [x] 1.1 (P) 送信パスと背圧エラーを実装する
  - outbound をキューへ投入し、送信失敗時に明確なエラーを返す
  - 送信先 authority が不正な場合の失敗扱いを統一する
  - _Requirements: 1.1, 1.2, 3.4_

- [x] 1.2 (P) 受信パスと空キュー時の挙動を実装する
  - 受信データを MembershipDelta に復元し、受信元と組にして返す
  - 受信キューが空のときは空の結果を返し続ける
  - _Requirements: 1.3, 1.4_

- [x] 1.3 (P) wire 形式と MembershipDelta の相互変換を実装する
  - status の変換と未知値の破棄を明確化する
  - 破壊的変更を許容する前提でデコード失敗を扱う
  - _Requirements: 1.3_

- [x] 2. Tokio gossiper のライフサイクルと周期駆動を実装する
- [x] 2.1 (P) start/stop による起動停止の制御を実装する
  - 起動失敗時はエラーを返す
  - 停止失敗時はエラーを返す
  - _Requirements: 2.1, 2.2, 2.4, 2.5_

- [x] 2.2 (P) 起動中の周期処理で gossip の送受信と更新を進める
  - 受信データを協調処理へ渡し、トポロジ更新を発行する
  - outbound を transport 経由で送信する
  - _Requirements: 2.3, 3.1, 3.2, 3.3_

- [x] 3. std/no_std 境界を維持した利用可能化を行う
- [x] 3.1 std 機能で tokio gossip を利用可能にする
  - std 構成で利用者が gossiper と transport を選べるようにする
  - _Requirements: 5.1_

- [x] 3.2 no_std 構成で tokio 依存が発生しないことを確認する
  - no_std のビルド境界を維持し、依存を持ち込まない
  - _Requirements: 5.2_

- [x] 4. Tokio gossip サンプルを提供する
  - 2 ノードの join/leave を実行し、TopologyUpdated を確認する
  - 実行が成功したことを確認できる出力を用意する
  - _Requirements: 4.1, 4.2, 4.3_

- [x] 5. テストと検証を整備する
- [x] 5.1 (P) transport と gossiper の単体テストを追加する
  - 送信失敗や空キュー時の挙動を検証する
  - start/stop の遷移を検証する
  - _Requirements: 5.3_

- [x] 5.2 (P) 2 ノードの統合テストでトポロジ更新を検証する
  - gossip 受信から更新通知までの流れを確認する
  - _Requirements: 5.3, 4.2_

- [x] 5.3 no_std/std のビルドが成功することを確認する
  - std と no_std の両方でビルドが通ることを確認する
  - _Requirements: 5.4_
