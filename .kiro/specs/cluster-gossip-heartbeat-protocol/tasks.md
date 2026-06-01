# 実装計画

- [x] 1. gossip envelope の core contract を追加する
  - gossip payload が from/to `UniqueAddress`、payload kind、membership version、deadline を保持できるようにする。
  - delta、full gossip state、seen digest、heartbeat request / response、cross-DC heartbeat を payload kind として区別できるようにする。
  - identity 未確定と deadline expired が caller から観測できる outcome になる。
  - envelope の unit test で identity、payload kind、deadline の完了状態を確認できる。
  - _Requirements: 1.1, 1.2, 1.3, 1.4_
  - _Boundary: GossipEnvelope_

- [x] 2. full gossip state merge と tombstone を membership core に追加する
  - full gossip state が membership snapshot、reachability snapshot、tombstone set を同じ merge unit として扱えるようにする。
  - 同じ member identity の conflict が deterministic precedence rule で解決されるようにする。
  - removed/dead member の tombstone が stale member reappearance を抑止し、retention 条件で prune できる。
  - merge と tombstone の unit test で input order に依存しない result と prune 条件を確認できる。
  - _Requirements: 2.1, 2.2, 2.3, 2.4_
  - _Boundary: GossipStateModel_
  - _Depends: 1_

- [x] 3. seen digest と convergence 判定を coordinator に接続する
  - peer identity ごとの observed version を seen digest として保持する。
  - delta diffusion と full gossip merge の両方で seen digest が更新されるようにする。
  - active peer 全員が対象 version を確認したときに convergence event が観測できる。
  - coordinator test で seen digest update、convergence、peer set 更新後の retention を確認できる。
  - _Requirements: 2.5, 2.6_
  - _Boundary: GossipStateModel_
  - _Depends: 2_

- [x] 4. dedicated heartbeat request / response protocol を追加する
  - peer ごとの sequence number と pending request を持つ heartbeat protocol state を追加する。
  - heartbeat request から response を生成し、response を request と照合して liveness evidence に変換する。
  - first heartbeat expectation と通常 timeout が missed heartbeat evidence として観測できる。
  - heartbeat evidence が reachability input だけを生成し、downing decision を実行しないことを test で確認できる。
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_
  - _Boundary: HeartbeatProtocolState_
  - _Depends: 1_

- [ ] 5. Cross-DC heartbeat evidence を追加する
  - membership snapshot の data center を使い、same data center と cross-DC target を区別できるようにする。
  - cross-DC heartbeat request / response が local heartbeat と区別できる kind と data center pair を保持する。
  - membership 更新により cross-DC target の追加、削除、維持が観測できる。
  - Cross-DC evidence が routing、discovery、downing strategy を決定しないことを test で確認できる。
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_
  - _Boundary: CrossDcHeartbeat_
  - _Depends: 4_

- [ ] 6. std transport handoff を envelope-aware にする
  - std transport handoff が payload kind、from/to identity、peer mapping を保持するようにする。
  - unknown payload kind、invalid identity、unknown peer が transport failure として観測できる。
  - `TokioGossipTransport` が endpoint mapping と envelope roundtrip を保持し、core merge semantics を所有しない。
  - std adaptor integration test で gossip state payload と heartbeat payload を区別した roundtrip を確認できる。
  - _Requirements: 1.5, 5.1, 5.2, 5.3, 5.4, 5.5_
  - _Boundary: GossipTransportHandoff, TokioGossipTransport_
  - _Depends: 1, 3, 4, 5_

- [ ] 7. gap analysis evidence と scope guard を更新する
  - `GossipEnvelope`、dedicated cluster heartbeat protocol、full `Gossip` merge / tombstone / seen digest、`CrossDcClusterHeartbeat` の evidence を `docs/gap-analysis/cluster-gap-analysis.md` に反映する。
  - downing SBR、discovery provider、pubsub mediator、serialization contract、Deferred Pekko concepts をこの task で完了扱いにしない。
  - targeted tests と no_std check の結果で gossip / heartbeat protocol の境界が成立することを確認する。
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_
  - _Boundary: ScopeGuard, GapAnalysisUpdate_
  - _Depends: 6_
