# 実装計画

- [x] 1. SeedNodeProcess の core contract を追加する
  - seed authority を provider lifecycle から受け取り、member mode と client mode の違いを観測できる contract を定義する。
  - empty seed、self authority、duplicate authority、invalid authority、shutdown 後入力停止を unit test で確認する。
  - 完了時には seed node source から join input を生成する最小 surface が `no_std` core で参照できる。
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 3.1, 3.2_
  - _Boundary: SeedNodeProcess_

- [x] 2. provider-neutral discovery result model を追加する
  - backend-neutral な discovered authority、source identity、observation time、empty result、failure result を表現する。
  - provider-specific metadata が placement / membership input として公開されないことを test で確認する。
  - 完了時には generic backend result を cluster core が読める最小 value contract に正規化できる。
  - _Requirements: 2.1, 2.2, 2.4, 4.2_
  - _Boundary: DiscoveredAuthority, DiscoveryResult_

- [x] 3. DiscoveryTopologyMapper を追加する
  - discovery result から joined / left の差分だけを topology update として生成する。
  - duplicate authority の dedup、failure 時に既存 topology を破壊しないこと、block list contract を維持することを unit test で確認する。
  - 完了時には static seed / generic discovery / AWS ECS result が同じ topology update contract に変換される。
  - _Requirements: 1.1, 2.2, 2.3, 3.3, 4.1, 4.3_
  - _Boundary: DiscoveryTopologyMapper_
  - _Depends: 2_

- [x] 4. std generic discovery backend contract を追加する
  - std adaptor 側で backend execution を表す trait と observable failure error を定義する。
  - polling または subscription の入力が `DiscoveryResult` に変換されることを fake backend test で確認する。
  - 完了時には特定 cloud provider に依存しない discovery backend bridge を追加できる。
  - _Requirements: 2.1, 2.2, 2.5_
  - _Boundary: GenericDiscoveryAdapter_
  - _Depends: 2, 3_

- [ ] 5. provider lifecycle bridge を実装する
  - member start では seed/discovery を join input に変換し、client start では full member 自己登録を生成しないようにする。
  - shutdown で polling/subscription が停止し、provider を strong reference で生存させないことを std test で確認する。
  - 完了時には provider lifecycle と discovery lifecycle の停止境界が観測できる。
  - _Requirements: 1.5, 3.1, 3.2, 3.4, 3.5_
  - _Boundary: ProviderLifecycleBridge, GenericDiscoveryAdapter_
  - _Depends: 1, 4_

- [ ] 6. 既存 Local / static / AWS ECS provider との interop を確認する
  - Local provider の seed node input が SeedNodeProcess bridge を通じて topology update になることを確認する。
  - Static provider が discovery polling を開始しない既存 contract を維持することを確認する。
  - AWS ECS provider の existing polling behavior を壊さず、generic mapping contract と矛盾しないことを確認する。
  - 完了時には既存 provider の public behavior を維持したまま discovery interop contract が追加される。
  - _Requirements: 2.5, 4.1, 4.3_
  - _Boundary: SeedNodeProcessBridge, GenericDiscoveryAdapter_
  - _Depends: 3, 5_

- [ ] 7. 隣接 spec の scope を吸収していないことを検証する
  - membership reachability、WeaklyUp、gossip heartbeat、downing/SBR、pubsub、serialization の実装をこの feature に含めていないことを差分で確認する。
  - discovery result の source identity が placement / membership policy の入力になっていないことを確認する。
  - 完了時には provider/discovery interop の boundary が roadmap の downstream spec と分離されたままになる。
  - _Requirements: 4.2, 4.4, 5.1, 5.2, 5.3, 5.4_
  - _Boundary: ScopeGuard_
  - _Depends: 6_

- [ ] 8. gap analysis evidence と targeted verification を更新する
  - `SeedNodeProcess` と generic discovery adapter の evidence だけを `docs/gap-analysis/cluster-gap-analysis.md` に反映する。
  - Deferred Pekko concepts と downstream spec 項目を完了扱いにしないことを確認する。
  - 対象 crate の unit tests / std tests / no-std check を実行し、provider/discovery interop の contract が同時に成立することを確認する。
  - _Requirements: 5.5_
  - _Boundary: GapAnalysisUpdate_
  - _Depends: 7_
