# 実装計画

- [ ] 1. cluster message serialization の core contract を追加する
- [x] 1.1 payload kind と manifest preservation rule を定義する
  - gossip と pubsub を stable payload kind として表し、unknown raw tag を既知 kind に丸めない。
  - actor-core manifest を opaque に保持し、payload kind tag と二重管理しないことを検証する。
  - unknown manifest / manifest route failure は actor-core deserialize failure として観測可能にし、cluster 固有 fallback に変換しない。
  - 完了時には kind tag stability、unknown kind、manifest preservation rule、unknown manifest route failure の unit test が通る。
  - _Requirements: 2.1, 2.4, 2.5, 4.2, 4.4_
  - _Boundary: ClusterMessagePayloadKind, ClusterMessageManifest_

- [x] 1.2 actor-core serialized metadata を保持する bridge message を定義する
  - payload kind と actor-core `SerializedMessage` を束ねる immutable value を追加する。
  - serializer id、manifest、payload bytes が constructor と accessor で欠落なく観測できるようにする。
  - 完了時には manifest あり / なしの metadata preservation test が通る。
  - _Requirements: 1.1, 1.3, 1.4, 3.2_
  - _Boundary: ClusterSerializedMessage_

- [ ] 2. actor-core serialization との接続点を実装する
- [x] 2.1 `SerializationExtension` を使う cluster bridge を追加する
  - cluster payload kind、`SerializationCallScope`、typed payload を受け取り、actor-core serialization の結果を `ClusterSerializedMessage` として返す。
  - wire bridge caller は `SerializationCallScope::Remote` を渡し、manifest-required scope を Local 扱いで迂回しない。
  - 未登録 serializer、serialize failure、deserialize failure を cluster 独自 fallback に変換しない。
  - 完了時には custom serializer の serializer id / manifest が bridge roundtrip 後も保持され、Remote scope の manifest requirement test が通る。
  - _Requirements: 1.1, 1.2, 1.3, 4.3, 5.5_
  - _Boundary: ActorSerializationBridge_

- [ ] 3. std/wire frame と codec を追加する
- [x] 3.1 versioned cluster wire frame を定義する
  - version、payload kind tag、serializer id、manifest、payload length、payload bytes を含む v1 frame を追加する。
  - frame は endpoint、association、retry state を持たない。
  - 完了時には v1 frame encode/decode が metadata を保持して roundtrip する。
  - _Requirements: 3.1, 3.2, 3.4, 5.3_
  - _Boundary: ClusterWireFrameV1_

- [ ] 3.2 decode failure taxonomy と malformed payload rejection を実装する
  - unknown version、unknown payload kind、unknown manifest、length mismatch、invalid manifest bytes を区別する。
  - decode failure 時に actor message、empty message、dead-letter message へ変換しない。
  - 完了時には各 failure category の unit test が個別に失敗種別を検証する。
  - _Requirements: 3.3, 4.1, 4.2, 4.4, 5.4_
  - _Boundary: ClusterWireCodec, ClusterWireDecodeFailure_

- [ ] 4. upstream gossip/pubsub payload と境界検証を接続する
- [ ] 4.1 (P) gossip payload の serialization bridge smoke test を追加する
  - upstream gossip payload contract 由来の値を payload kind `Gossip` として serialized message に包む。
  - bridge は gossip merge、seen digest、heartbeat evidence、reachability update を実行しないことを検証する。
  - 完了時には gossip payload の wire roundtrip smoke test が semantics evaluation なしで通る。
  - _Requirements: 2.2, 5.1_
  - _Boundary: ScopeGuard, ActorSerializationBridge_
  - _Depends: 1.2, 2.1, 3.2_

- [ ] 4.2 (P) pubsub payload の serialization bridge smoke test を追加する
  - upstream pubsub payload contract 由来の値を payload kind `PubSub` として serialized message に包む。
  - bridge は mediator command application、delivery target selection、registry delta application を実行しないことを検証する。
  - 完了時には pubsub payload の wire roundtrip smoke test が mediator state mutation なしで通る。
  - _Requirements: 2.3, 5.2_
  - _Boundary: ScopeGuard, ActorSerializationBridge_
  - _Depends: 1.2, 2.1, 3.2_

- [ ] 5. integration evidence と gap analysis を更新する
- [ ] 5.1 serializer contract follow-up の evidence を更新し、targeted checks を実行する
  - `docs/gap-analysis/cluster-gap-analysis.md` の cluster message serializer contract 項目だけを implementation evidence で更新する。
  - protobuf / Pekko binary compatibility は scope 外として残し、別 follow-up に吸収しない。
  - 完了時には targeted unit/integration tests と `cluster-core-kernel` no_std check が通る。
  - _Requirements: 4.5, 5.3, 5.4, 5.5_
  - _Boundary: GapAnalysisUpdate, ScopeGuard_
  - _Depends: 4.1, 4.2_
