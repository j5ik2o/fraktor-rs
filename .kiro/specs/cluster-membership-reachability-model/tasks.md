# 実装計画

- [x] 1. member identity と data center の土台を追加する
  - membership record が address + uid の identity を保持し、同じ address の別 uid を別 incarnation として扱えるようにする。
  - default data center と explicit data center を区別できる membership primitive を追加する。
  - cluster core が remote identity primitive を使っても `no_std` 境界を維持できることを確認する。
  - identity と data center が snapshot / delta で観測できる。
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3_
  - _Boundary: NodeIdentityRecord, DataCenterMembership_

- [x] 2. membership table と current state に data center view を接続する
  - join/rejoin の membership flow が identity と data center を保持するようにする。
  - data center ごとの member view が status と identity を失わずに取得できるようにする。
  - current cluster state が data center 付き member view を公開し、Cross-DC heartbeat は開始しない。
  - data center filtering の完了状態を unit test で観測できる。
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_
  - _Boundary: DataCenterMembership_
  - _Depends: 1_

- [x] 3. WeaklyUp status と transition rule を追加する
  - membership status に `WeaklyUp` 相当を追加し、`Joining -> WeaklyUp -> Up` を観測できるようにする。
  - `WeaklyUp` から leave/down/remove へ進む transition rule を定義する。
  - active member view で暫定参加を caller が判定できる helper を用意する。
  - `WeaklyUp` が SBR decision を実行しないことを test で確認できる。
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_
  - _Boundary: WeaklyUpStatus_
  - _Depends: 1_

- [x] 4. Reachability matrix を membership core に追加する
  - observer / subject / status / version を持つ reachability record を保持できるようにする。
  - unreachable、reachable、terminated update と observer version の進行を実装する。
  - reachable が default 状態の場合は不要 record が残らず、terminated が unreachable より強い aggregate status になることを確認する。
  - reachability snapshot が matrix records と versions を保持することを unit test で観測できる。
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_
  - _Boundary: ReachabilityMatrix_

- [x] 5. indirect connection evidence を downing input 境界へ渡せるようにする
  - partial connectivity を direct observation と indirect observation に分けて表現する。
  - subject と observer 自身の aggregate reachability を使い、indirect connection evidence を生成できるようにする。
  - evidence が存在しない場合は direct reachability evidence だけで表現する。
  - downing boundary には evidence だけが渡り、downing decision や lease majority 判定が発生しないことを確認する。
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_
  - _Boundary: IndirectConnectionEvidence_
  - _Depends: 4_

- [x] 6. membership coordinator と snapshots の統合を検証する
  - failure detector や heartbeat receipt からの reachability 変化が matrix と status transition の両方に反映されるようにする。
  - membership snapshot と current cluster state が identity、data center、WeaklyUp、reachability snapshot を同時に保持することを確認する。
  - downstream specs が参照できる public surface を `membership` module から最小限公開する。
  - 対象 crate の membership unit tests が一連の identity/status/reachability flow を通して成立する。
  - _Requirements: 1.4, 2.3, 3.2, 4.5, 5.4_
  - _Boundary: NodeIdentityRecord, DataCenterMembership, WeaklyUpStatus, ReachabilityMatrix, IndirectConnectionEvidence_
  - _Depends: 2, 3, 5_

- [x] 7. gap analysis evidence と scope guard を更新する
  - active medium の5項目について、実装済みまたは core contract 化済みの evidence を `docs/gap-analysis/cluster-gap-analysis.md` に反映する。
  - gossip/heartbeat、SBR/downing strategy、discovery、pubsub、serialization、Deferred Pekko concepts をこの task で完了扱いにしない。
  - targeted tests と no_std check の結果で membership/reachability model の境界が成立することを確認する。
  - _Requirements: 6.1, 6.2, 6.3, 6.4_
  - _Boundary: ScopeGuard, GapAnalysisUpdate_
  - _Depends: 6_
