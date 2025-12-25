# Implementation Plan

- [ ] 1. 基盤型と ask 応答契約の Result 化
- [ ] 1.1 ask の失敗理由を統一し、結果を Result として扱える型を整備する
  - 成功値と失敗理由が型で区別されることを保証する
  - タイムアウト/配送不能/送信失敗を失敗理由として扱えることを整理する
  - _Requirements: 1.1,1.2,3.1,3.2,3.3_
- [ ] 1.2 ask の完了経路で Result を完了し、失敗時の通知を維持する
  - 失敗時の EventStream 通知が従来どおり行われることを確認する
  - 成功と失敗が混在しない完了経路であることを確認する
  - _Requirements: 1.1,1.2,1.3,3.1,3.2,3.3_

- [ ] 2. Typed ask の Result 伝搬
- [ ] 2.1 typed 側の ask 応答ハンドルを Result で扱えるよう整備する
  - reply_to モデルを維持したまま成功/失敗を Result として扱えることを確認する
  - untyped との変換が一貫することを確認する
  - _Requirements: 2.1,2.2,2.3_

- [ ] 3. Cluster request 経路の Result 化
- [ ] 3.1 cluster の request/timeout 経路を Result に統一し、失敗理由を変換できるようにする
  - 失敗理由が Result の Err として返ることを保証する
  - EventStream の失敗通知が維持されることを確認する
  - _Requirements: 1.2,1.3,3.1,3.2,3.3_
- [ ] 3.2 ask 返信とエラーの混在を解消し、request_future の利用経路を整理する
  - 既存の利用箇所で成功/失敗の扱いが明確になるよう整備する
  - _Requirements: 1.1,1.2,2.1,2.3_

- [ ] 4. テストと利用例の更新
- [ ] 4.1 ask 成功/失敗の Result を検証するテストを追加・更新する
  - タイムアウト/配送不能/送信失敗の Result を検証する
  - _Requirements: 1.1,1.2,3.1,3.2,3.3_
- [ ] 4.2 Result 化された ask の利用例を更新する
  - 既存例で Result の成功/失敗を扱う流れを示す
  - _Requirements: 4.1,4.2_
