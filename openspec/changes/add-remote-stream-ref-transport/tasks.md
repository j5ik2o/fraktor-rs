## 1. 実装リセット

- [ ] 1.1 途中までの remote StreamRef 実装差分を確認し、破棄対象と残す文書差分を切り分ける。
- [ ] 1.2 破棄対象の実装コード、テスト、Cargo 依存、再エクスポートを戻す。
- [ ] 1.3 破棄後に `git status --short` で OpenSpec 文書以外の不要差分が残っていないことを確認する。
- [ ] 1.4 破棄後のベースラインで、既存 StreamRef 関連テストが元の状態で通ることを確認する。

## 2. ベースラインと契約確認

- [ ] 2.1 Pekko `StreamRefs.sourceRef` / `StreamRefs.sinkRef` / `StreamRefResolverImpl` / `SourceRefImpl` / `SinkRefImpl` / `StreamRefsMaster` と remote StreamRef examples を読み直す。
- [ ] 2.2 `SourceRef` は `SourceRef` として resolve し、`SinkRef` は `SinkRef` として resolve する、という Pekko 互換の向きをテスト名または短いコメントで固定する。
- [ ] 2.3 現行 `stream-core-kernel` の local `SourceRef` / `SinkRef`、protocol、settings、terminal ordering、handoff logic を確認する。
- [ ] 2.4 endpoint state change が stream materializer を再駆動できる既存経路を確認する。
- [ ] 2.5 local materialization contract が固まった後にのみ、remote provider dispatch、remote watch hook、serialization registry、TCP outbound envelope path を確認する。

## 3. Public StreamRef Materialization Contract

- [ ] 3.1 producer stream を materialize して `SourceRef<T>` を得る `StreamRefs.source_ref` 相当の contract を定義する。
- [ ] 3.2 consumer sink を materialize して `SinkRef<T>` を得る `StreamRefs.sink_ref` 相当の contract を定義する。
- [ ] 3.3 public API 名、テスト名、サンプルに `spawn_source_ref -> resolve_sink_ref` のような逆向き workflow を出さない。
- [ ] 3.4 actor path string と endpoint actor detail は resolver / serializer support API に閉じ、application-level workflow の中心にしない。
- [ ] 3.5 `stream-core-kernel` の no_std と remote 非依存を維持する。

## 4. Local Resolver Round-Trip Gate

- [ ] 4.1 producer stream から materialized `SourceRef<T>` を作り、serialization format 経由で `SourceRef<T>` として resolve し、要素と completion が届く non-ignored local test を追加する。
- [ ] 4.2 consumer sink から materialized `SinkRef<T>` を作り、serialization format 経由で `SinkRef<T>` として resolve し、producer stream から要素と completion が届く non-ignored local test を追加する。
- [ ] 4.3 demand 到着前に upstream が ready になっても accepted element を失わないことを検証する。
- [ ] 4.4 completion が accepted sequenced elements より先に観測されないことを検証する。
- [ ] 4.5 failure と cancellation が paired endpoint へ伝播し、materialized stream が stall しないことを検証する。
- [ ] 4.6 4.1 と 4.2 が通常テストとして pass するまで remote two-ActorSystem integration へ進まない。

## 5. Resolver and Serializer Support

- [ ] 5.1 materialized `SourceRef<T>` / `SinkRef<T>` を actor-core path API で canonical endpoint actor path string に変換する resolver support を実装する。
- [ ] 5.2 serialized `SourceRef` は `SourceRef<T>` へ、serialized `SinkRef` は `SinkRef<T>` へ ActorSystem provider dispatch 経由で resolve する。
- [ ] 5.3 canonical authority、loopback resolution、remote authority resolution、unsupported ref implementation の lower-level format tests を追加する。
- [ ] 5.4 typed `SourceRef<T>` / `SinkRef<T>` を domain message payload として扱える serializer support を追加する。
- [ ] 5.5 remote endpoint communication に必要な StreamRef protocol payload serializer または manifest route を登録する。
- [ ] 5.6 missing serializer registration と unsupported payload manifest の failure tests を追加する。

## 6. Remote Endpoint Actor Wiring

- [ ] 6.1 SourceRef / SinkRef endpoint actor を materialized stream resource として所有し、completion / cancellation / failure 時に deterministic shutdown する。
- [ ] 6.2 cumulative demand、sequenced element、handshake、completion、failure、ack、cancellation を通常の remote ActorRef delivery で配送する。
- [ ] 6.3 one-shot partner pairing を守り、double materialization と non-partner message を observable failure にする。
- [ ] 6.4 endpoint partner watch を既存 actor watch path と remote watch hook に接続する。
- [ ] 6.5 watch release failure、send failure、endpoint shutdown failure、transport enqueue failure を握りつぶさず観測可能にする。
- [ ] 6.6 endpoint actor state change が materialized stream を wake / drive し、handshake / demand / terminal update で stall しないことを保証する。

## 7. Backpressure and Failure Semantics

- [ ] 7.1 cumulative stream-level demand なしに element が配送されないことを検証する。
- [ ] 7.2 transport enqueue backpressure 時に accepted element を保持するか、observable transport error で stream を fail させる。
- [ ] 7.3 pending sequenced elements が配送された後にのみ completion が観測されることを検証する。
- [ ] 7.4 partner DeathWatch、address termination、transport connection loss、invalid sequence、invalid demand、invalid partner、duplicate materialization を distinct observable stream failures に写像する。
- [ ] 7.5 cancellation が remote partner へ伝播し、その ref への追加 publication を止めることを検証する。

## 8. Integration Tests and Documentation

- [ ] 8.1 typed `SourceRef<T>` を remote message payload として渡し、two-ActorSystem 間で backpressure 付き stream が流れる integration test を追加する。
- [ ] 8.2 typed `SinkRef<T>` を remote message payload として渡し、two-ActorSystem 間で backpressure 付き stream が流れる integration test を追加する。
- [ ] 8.3 partner termination、address termination、transport connection loss が protocol completion 前に stream failure として観測される remote failure integration tests を追加する。
- [ ] 8.4 completion criteria を表す regression tests から `#[ignore]` を外す。
- [ ] 8.5 local と two-ActorSystem の StreamRef proof が通った後にのみ `docs/gap-analysis/stream-gap-analysis.md` を更新する。
- [ ] 8.6 targeted crate tests、`mise exec -- openspec validate add-remote-stream-ref-transport --strict`、`git diff --check` を実行する。
- [ ] 8.7 明示的に狭い検証範囲が承認されていない限り、完了前に `./scripts/ci-check.sh ai all` を実行する。

### Verification Gate

この task list は途中実装を未完了として扱い、実装を一旦破棄してから再開する前提でリセット済みである。PR 可能条件は、local resolver round-trip tests と two-ActorSystem typed `SourceRef` / `SinkRef` payload tests が `#[ignore]` なしで pass することである。
