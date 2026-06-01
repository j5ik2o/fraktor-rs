# 実装計画

- [x] 1. compatibility key catalog の土台を追加する
  - join compatibility で比較する required key、比較対象外の sensitive/local-only key、downstream が参照する stable key 名を定義する。
  - 既存の pubsub、downing provider、SBR settings に加えて failure detector choice の identity を catalog に含める。
  - catalog は `no_std` core で使える immutable surface として観測できる。
  - _Requirements: 1.1, 1.3, 1.5_
  - _Boundary: ClusterCompatibilityKeyCatalog_

- [x] 2. checker composition と config validation を拡張する
  - `ClusterExtensionConfig` の join compatibility が catalog の required/excluded key semantics に従うようにする。
  - 複数 checker の incompatible reason が失われない形で合成されることを unit test で確認する。
  - sensitive/local-only key を変えても compatibility result が変わらないことを観測できる。
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_
  - _Boundary: JoinCompatibilityComposition_
  - _Depends: 1_

- [x] 3. remote actor path helper contract を固定する
  - local actor ref が cluster advertised authority 付き canonical remote path に変換されることを維持する。
  - 既存 remote authority と UID が保持され、canonical path 不在時は observable error になることを test で確認する。
  - `ClusterApi::remote_path_of` の scope を path formatting に限定し、remote delivery や actor resolution behavior を変更しない。
  - _Requirements: 2.1, 2.2, 2.3, 2.4_
  - _Boundary: RemotePathHelper_

- [x] 4. downing provider compatibility metadata を固定する
  - provider key と SBR settings identity を `DowningProviderCompatibility` の比較語彙として観測できるようにする。
  - custom provider factory と no-op downing provider の既存 behavior を変更しないことを確認する。
  - provider-facing SBR hook、decision failure 変換、reachability、lease majority の判定は downstream `cluster-downing-sbr-decision-model` に残す。
  - _Requirements: 3.1, 3.2, 3.3, 3.4_
  - _Boundary: DowningProviderCompatibility_

- [x] 5. std remoting lifecycle bridge retention を検証する
  - `subscribe_remoting_events` が `EventStreamSubscription` を返し、helper return 後も guard 保持中の connected event が topology join input になることを確認する。
  - guard drop 後に topology update が生成されないことを確認する。
  - subscription が provider を strong reference で保持しないことを確認する。
  - _Requirements: 4.1, 4.2, 4.3, 4.4_
  - _Boundary: TransportLifecycleBridge_

- [ ] 6. baseline 統合と gap analysis evidence を更新する
  - trivial / easy の4項目について、実装済みまたは baseline 契約化済みの evidence を `docs/gap-analysis/cluster-gap-analysis.md` に反映する。
  - membership、gossip、downing decision、discovery、pubsub、serialization、Deferred Pekko concepts の status をこの task で完了扱いにしない。
  - 対象 crate の unit tests を実行し、config/path/provider/lifecycle の baseline が同時に成立することを確認する。
  - _Requirements: 5.1, 5.2, 5.3_
  - _Boundary: GapAnalysisUpdate_
  - _Depends: 2, 3, 4, 5_
