# 実装タスク

- [ ] 1. Failure Detector Configuration (故障検出器設定) の基盤を作る
- [x] 1.1 `FailureDetectorConfigError` で Cluster Configuration Validation (クラスタ設定検証) の失敗理由を表現する
  - phi threshold、max sample size、min standard deviation、first heartbeat estimate の不成立を、それぞれ区別できる error として扱う。
  - error は Join Compatibility (参加互換性) の不一致理由ではなく、設定値そのものの validation failure として使える状態にする。
  - 完了時点で、各 invalid field category を単体テストで識別できる。
  - _Requirements: 3.2, 3.3, 3.4, 3.6_
  - _Boundary: FailureDetectorConfigError_

- [ ] 1.2 `FailureDetectorConfig` が観測パラメータと default を保持する
  - phi threshold、max sample size、min standard deviation、acceptable heartbeat pause、first heartbeat estimate を保持する。
  - default は既存 cluster membership path と同等の `1.0`, `10`, `1ms`, `0ms`, `10ms` にする。
  - suspect timeout、dead timeout、quarantine ttl、gossip interval は含めない。
  - 完了時点で、default と custom value が getter から観測でき、Duration を単位付き値として保持していることを単体テストで確認できる。
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 2.4_
  - _Boundary: FailureDetectorConfig_

- [ ] 1.3 `FailureDetectorConfig` の validation と差分抽出を完成させる
  - 正の有限値ではない phi threshold、0 の max sample size、0 の min standard deviation、0 の first heartbeat estimate を拒否する。
  - acceptable heartbeat pause の 0 は valid として扱う。
  - 差分のある観測パラメータ名だけを Compatibility Mismatch Reason (互換性不一致理由) の detail に使える形で返す。
  - 完了時点で、valid / invalid の境界と差分 field 名が単体テストで観測できる。
  - _Requirements: 3.2, 3.3, 3.4, 3.5, 4.5_
  - _Boundary: FailureDetectorConfig_
  - _Depends: 1.1, 1.2_

- [ ] 2. Cluster Configuration (クラスタ設定) と Join Compatibility (参加互換性) へ接続する
- [ ] 2.1 (P) `ClusterCompatibilityKeyCatalog` に `cluster.failure-detector` を追加する
  - Failure Detector Configuration (故障検出器設定) 用の required key を追加する。
  - `cluster.failure-detector.choice` は excluded key のまま維持し、required / conditional に入れない。
  - 完了時点で、required keys と excluded keys の分類を単体テストで確認できる。
  - _Requirements: 1.4, 4.4, 4.6, 6.2_
  - _Boundary: ClusterCompatibilityKeyCatalog_
  - _Depends: 1.1_

- [ ] 2.2 (P) `ClusterExtensionConfig` が Failure Detector Configuration (故障検出器設定) を保持する
  - Cluster Configuration (クラスタ設定) の一部として `FailureDetectorConfig` を保持する。
  - 明示指定がない場合は `FailureDetectorConfig` の default を使う。
  - custom config を builder-style API で設定でき、getter で同じ値を取得できる。
  - 完了時点で、default config と custom config が `ClusterExtensionConfig` のテストから観測できる。
  - _Requirements: 1.1, 1.2, 1.3, 3.1_
  - _Boundary: ClusterExtensionConfig_
  - _Depends: 1.2_

- [ ] 2.3 `ClusterExtensionConfig` の Join Compatibility (参加互換性) 判定に Failure Detector Configuration (故障検出器設定) を含める
  - local と joining の Failure Detector Configuration (故障検出器設定) が一致する場合は compatible として扱う。
  - 不一致の場合は `cluster.failure-detector` の single key で incompatible reason を作る。
  - reason detail には差分のある観測パラメータ名を含め、`cluster.failure-detector.choice` を必須 key として扱わない。
  - 完了時点で、一致時 accepted / 不一致時 rejected / choice key excluded が統合テストで確認できる。
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6_
  - _Boundary: ClusterExtensionConfig, ClusterCompatibilityKeyCatalog_
  - _Depends: 1.3, 2.1, 2.2_

- [ ] 3. install / start 境界で Cluster Configuration Validation (クラスタ設定検証) を実行する
- [ ] 3.1 `ClusterError` が Cluster Configuration Validation (クラスタ設定検証) を表せるようにする
  - start member / client で返す validation failure を cluster lifecycle error として扱えるようにする。
  - error は Join Compatibility (参加互換性) failure と区別できる形にする。
  - 完了時点で、Failure Detector Configuration (故障検出器設定) の validation failure を lifecycle error として比較できる。
  - _Requirements: 3.1, 3.6_
  - _Boundary: ClusterError_
  - _Depends: 1.1_

- [ ] 3.2 `ClusterCore` が start 前に Failure Detector Configuration (故障検出器設定) を検証する
  - `ClusterCore` は start member / client の前提として Failure Detector Configuration (故障検出器設定) を保持する。
  - provider、pubsub、gossiper の start より前に validation failure を返す。
  - validation failure は Join Compatibility (参加互換性) failure ではなく cluster lifecycle error として観測できる。
  - 完了時点で、invalid config では provider 側 start へ進まないことをテストで確認できる。
  - _Requirements: 3.1, 3.6_
  - _Boundary: ClusterCore_
  - _Depends: 2.2, 3.1_

- [ ] 3.3 (P) `ClusterExtensionInstaller` が install 前に Failure Detector Configuration (故障検出器設定) を検証する
  - extension install 時に `ClusterExtensionConfig` の validation を実行する。
  - invalid config は actor system build configuration failure として返す。
  - provider、pubsub、identity lookup の組み立て前に failure が観測できるようにする。
  - 完了時点で、invalid config の install が configuration error で止まることをテストで確認できる。
  - _Requirements: 3.1, 3.6_
  - _Boundary: ClusterExtensionInstaller_
  - _Depends: 2.2_

- [ ] 4. std 環境で Availability Evidence (可用性観測証拠) の観測へ接続する
- [ ] 4.1 (P) `FailureDetectorConfig` から Phi Accrual detector を生成する bridge を追加する
  - std 側で `FailureDetectorConfig` の値を `PhiAccrualFailureDetector` の constructor へ渡す。
  - Duration を millis に変換し、public な Cluster Configuration (クラスタ設定) 上の duration 意味を維持する。
  - bridge は fixed Phi Accrual 生成だけを担い、Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) の API を持たない。
  - 完了時点で、threshold、sample size、duration millis が生成後の detector から観測できる。
  - _Requirements: 5.1, 5.2, 5.3, 6.2_
  - _Boundary: ConfiguredPhiAccrualDetectorFactory_
  - _Depends: 1.2, 1.3_

- [ ] 4.2 std bridge を既存の detector registry 利用箇所へ接続する
  - 既存の ad hoc Phi Accrual constructor usage を、Cluster Configuration (クラスタ設定) 由来の bridge を使う形へ置き換える。
  - `MembershipCoordinatorConfig::phi_threshold` の既存互換性は壊さず、この feature の範囲では policy cleanup をしない。
  - 完了時点で、std 側の coordinator / gossiper test が Failure Detector Configuration (故障検出器設定) 由来の detector で通る。
  - _Requirements: 5.1, 5.2, 5.3_
  - _Boundary: ConfiguredPhiAccrualDetectorFactory, MembershipCoordinator integration_
  - _Depends: 3.2, 4.1_

- [ ] 5. gap analysis と scope guard を更新する
- [ ] 5.1 cluster gap analysis の該当項目を完了扱いへ更新する
  - Failure Detector Configuration (故障検出器設定) の contract、validation、Join Compatibility (参加互換性)、std bridge が揃ったことを反映する。
  - Split Brain Resolver execution actor、provider down execution loop、lease coordination backend はこの成果として書かない。
  - Cluster Singleton、Cluster Client、Receptionist、Distributed Data / CRDT、Pekko public API parity をこの feature の成果として扱わない。
  - 完了時点で、gap analysis からこの feature の完了範囲と延期範囲を区別して読める。
  - _Requirements: 6.1, 6.3, 6.4_
  - _Boundary: gap analysis docs_
  - _Depends: 2.3, 3.2, 3.3, 4.2_

- [ ] 6. feature 全体を検証する
- [ ] 6.1 対象 crate の単体・統合テストを通す
  - cluster-core の Failure Detector Configuration (故障検出器設定)、Join Compatibility (参加互換性)、install / start validation のテストを実行する。
  - cluster-adaptor-std の std bridge と既存 detector registry 接続のテストを実行する。
  - 完了時点で、対象 test command の成功結果が確認できる。
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 5.1, 5.2, 5.3_
  - _Boundary: Verification_
  - _Depends: 5.1_

- [ ] 6.2 no_std 境界、用語、禁止 scope を確認する
  - cluster-core に std 依存が増えていないことを確認する。
  - 禁止された用途語、旧 prefix の failure detector key、`cluster.failure-detector.choice` の required 化が残っていないことを検索で確認する。
  - docs が `CONTEXT.md` の用語に沿い、Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) を成果として扱っていないことを確認する。
  - 完了時点で、検索結果と targeted verification が scope guard を満たす。
  - _Requirements: 1.4, 2.3, 4.6, 5.3, 6.1, 6.2, 6.3, 6.4_
  - _Boundary: Verification_
  - _Depends: 6.1_
