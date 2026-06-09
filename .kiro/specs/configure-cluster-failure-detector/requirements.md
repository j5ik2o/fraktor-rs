# 要件ドキュメント

## 導入

fraktor-rs の cluster 実装者と運用者は、Failure Detector Configuration (故障検出器設定) を Cluster Configuration (クラスタ設定) として明示的に扱い、Availability Evidence (可用性観測証拠) の前提を Join Compatibility (参加互換性) で揃えたい。

現状は Failure Detector (故障検出器) の registry と Phi Accrual 実装はあるが、Cluster Configuration (クラスタ設定) から観測パラメータを設定する contract、Cluster Configuration Validation (クラスタ設定検証)、Join Compatibility (参加互換性) の単一 key、Compatibility Mismatch Reason (互換性不一致理由) が揃っていない。この feature は `FailureDetectorConfig` を導入し、現行の観測挙動を変えずに Phi Accrual 前提の観測パラメータを Cluster Configuration (クラスタ設定) から Failure Detector (故障検出器) の生成へ接続し、gap analysis を完了扱いへ更新する。

## 要件

### 1. Failure Detector Configuration (故障検出器設定)
**目的:** cluster 実装者として、Availability Evidence (可用性観測証拠) の観測方法を Cluster Configuration (クラスタ設定) として表現し、member 間の運用前提を明示したい。

#### 受け入れ条件
1. Cluster Configuration (クラスタ設定) が作成されたとき、fraktor-rs cluster は `FailureDetectorConfig` として Failure Detector Configuration (故障検出器設定) を保持しなければならない。
2. Failure Detector Configuration (故障検出器設定) が明示されない場合、fraktor-rs cluster は既存の Availability Evidence (可用性観測証拠) の観測挙動と同等の default を使わなければならない。
3. cluster 実装者が Failure Detector Configuration (故障検出器設定) の観測パラメータを指定したとき、fraktor-rs cluster は指定値を Cluster Configuration (クラスタ設定) の一部として保持しなければならない。
4. Failure Detector Configuration (故障検出器設定) を扱う場合、fraktor-rs cluster は Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) を利用者向け公開契約として要求してはならない。

### 2. 観測パラメータの範囲
**目的:** cluster 運用者として、Availability Evidence (可用性観測証拠) に直接影響する値だけを揃え、Membership Coordination Policy (メンバーシップ調停ポリシー) と混同しないようにしたい。

#### 受け入れ条件
1. Failure Detector Configuration (故障検出器設定) を含む場合、fraktor-rs cluster は Phi Accrual 前提の観測パラメータを設定対象に含めなければならない。
2. Phi Accrual 前提の観測パラメータを含む場合、fraktor-rs cluster は phi threshold、max sample size、min standard deviation、acceptable heartbeat pause、first heartbeat estimate を区別して扱わなければならない。
3. Failure Detector Configuration (故障検出器設定) を扱う場合、fraktor-rs cluster は suspect timeout、dead timeout、quarantine ttl、gossip interval を Failure Detector Configuration (故障検出器設定) に含めてはならない。
4. Duration を持つ観測パラメータを指定したとき、fraktor-rs cluster は Cluster Configuration (クラスタ設定) 上では単位付きの duration として扱わなければならない。

### 3. Cluster Configuration Validation (クラスタ設定検証)
**目的:** cluster 実装者として、不成立な Failure Detector Configuration (故障検出器設定) を Join Compatibility (参加互換性) より前に検出し、設定値そのものの誤りとして扱いたい。

#### 受け入れ条件
1. cluster extension の install / start が要求されたとき、fraktor-rs cluster は Failure Detector Configuration (故障検出器設定) の値を Cluster Configuration Validation (クラスタ設定検証) として検証しなければならない。
2. phi threshold が正の有限値ではない場合、fraktor-rs cluster は Cluster Configuration Validation (クラスタ設定検証) の失敗として報告しなければならない。
3. max sample size が 0 の場合、fraktor-rs cluster は Cluster Configuration Validation (クラスタ設定検証) の失敗として報告しなければならない。
4. min standard deviation または first heartbeat estimate が 0 の場合、fraktor-rs cluster は Cluster Configuration Validation (クラスタ設定検証) の失敗として報告しなければならない。
5. acceptable heartbeat pause が 0 の場合、fraktor-rs cluster はその値を有効な Failure Detector Configuration (故障検出器設定) として扱わなければならない。
6. Failure Detector Configuration (故障検出器設定) が不成立の場合、fraktor-rs cluster は Join Compatibility (参加互換性) の失敗ではなく Cluster Configuration Validation (クラスタ設定検証) の失敗として扱わなければならない。

### 4. Join Compatibility (参加互換性)
**目的:** cluster 運用者として、new Cluster Member (クラスタメンバー) を受け入れる前に Availability Evidence (可用性観測証拠) の前提が既存 member と一致していることを確認したい。

#### 受け入れ条件
1. new Cluster Member (クラスタメンバー) の join が要求されたとき、fraktor-rs cluster は Failure Detector Configuration (故障検出器設定) を Join Compatibility (参加互換性) の確認対象に含めなければならない。
2. 既存 member と new Cluster Member (クラスタメンバー) の Failure Detector Configuration (故障検出器設定) が一致する場合、fraktor-rs cluster は Failure Detector Configuration (故障検出器設定) を理由に join を拒否してはならない。
3. 既存 member と new Cluster Member (クラスタメンバー) の Failure Detector Configuration (故障検出器設定) が一致しない場合、fraktor-rs cluster は Join Compatibility (参加互換性) の失敗として join を拒否しなければならない。
4. Failure Detector Configuration (故障検出器設定) の不一致で join を拒否する場合、fraktor-rs cluster は Compatibility Mismatch Reason (互換性不一致理由) に単一 key `cluster.failure-detector` を含めなければならない。
5. Failure Detector Configuration (故障検出器設定) の不一致で join を拒否する場合、fraktor-rs cluster は Compatibility Mismatch Reason (互換性不一致理由) に差分のある観測パラメータ名を含めなければならない。
6. Join Compatibility (参加互換性) を確認する場合、fraktor-rs cluster は `cluster.failure-detector.choice` を必須 key として扱ってはならない。

### 5. Availability Evidence (可用性観測証拠) への反映
**目的:** cluster 運用者として、Cluster Configuration (クラスタ設定) に指定した Failure Detector Configuration (故障検出器設定) が実際の Availability Evidence (可用性観測証拠) の観測に反映されることを確認したい。

#### 受け入れ条件
1. std 環境で Failure Detector (故障検出器) が必要になったとき、fraktor-rs cluster は Cluster Configuration (クラスタ設定) の Failure Detector Configuration (故障検出器設定) に基づいて Availability Evidence (可用性観測証拠) を観測しなければならない。
2. Duration を持つ観測パラメータが Failure Detector (故障検出器) の観測に使われるとき、fraktor-rs cluster は public な Cluster Configuration (クラスタ設定) 上の duration 意味を維持しなければならない。
3. Failure Detector Configuration (故障検出器設定) が変更された場合、fraktor-rs cluster は Downing Decision (ダウン判断) や Member Removal (メンバー除去) の責務を Failure Detector (故障検出器) に追加してはならない。

### 6. Scope Boundary (スコープ境界)
**目的:** cluster 実装者として、この feature が Failure Detector Configuration (故障検出器設定) の公開契約化に閉じていることを確認し、延期スコープと混同しないようにしたい。

#### 受け入れ条件
1. この feature を含む場合、fraktor-rs cluster は Split Brain Resolver の実行 actor、provider からの実 down execution loop、具体的な lease coordination backend を追加対象に含めてはならない。
2. この feature を含む場合、fraktor-rs cluster は Failure Detector Algorithm Selection (故障検出器アルゴリズム選択) を追加対象に含めてはならない。
3. この feature が完了したとき、fraktor-rs project docs は cluster gap analysis の該当項目を完了扱いとして示さなければならない。
4. この feature が完了したとき、fraktor-rs project docs は Cluster Singleton、Cluster Client、Receptionist、Distributed Data / CRDT、Pekko public API parity をこの feature の成果として扱ってはならない。
