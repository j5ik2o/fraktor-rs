## 0. Change Rework Gate

- [x] 0.1 現行 change が考慮不足である理由を `problem-analysis.md` に固定する。
- [x] 0.2 既存の fake local resolver / fake serialization format / path-string-first 実装が残っていないことを確認する。
- [x] 0.3 この change を「remote transport 実装」ではなく「remote-capable StreamRef endpoint semantics + resolver/serializer wiring」として再定義する。

## 1. Baseline Contract Review

- [x] 1.1 Pekko `StreamRefs.sourceRef` / `StreamRefs.sinkRef` / `StreamRefResolverImpl` / `SourceRefImpl` / `SinkRefImpl` / `StreamRefsMaster` / `StreamRefSerializer` / remote StreamRef specs を、endpoint actor ownership と resolver direction の観点で読み直す。
- [x] 1.2 fraktor-rs 現行 `stream-core-kernel` の `SourceRef<T>` / `SinkRef<T>` が local handoff wrapper であることを確認し、remote-capable endpoint ref と同一型で扱う場合の追加表現を列挙する。
- [x] 1.3 `stream-adaptor-std`、`remote-adaptor-std`、`actor-core-kernel` の依存方向を確認し、endpoint actor / resolver / serializer を置ける crate 境界を決める。
- [x] 1.4 `stream-core-kernel` の no_std と remote 非依存を維持できない設計案を明示的に棄却する。

## 2. StreamRef Representation Decision

- [x] 2.1 `SourceRef<T>` / `SinkRef<T>` の内部表現を、local handoff backend と remote actor-backed backend のどちらも表せる形に決める。
- [x] 2.2 remote actor-backed backend が保持する最小データを決める: endpoint `ActorRef`、canonical path、type marker、one-shot state、materialized resource owner など。
- [x] 2.3 local-only ref を resolver serialization へ渡した場合の failure を定義する。fake actor path string を成功扱いにしない。
- [x] 2.4 `SourceRef` は `SourceRef` として resolve し、`SinkRef` は `SinkRef` として resolve する向きを API 名・テスト名・serializer helper 名で固定する。
- [x] 2.5 application-level examples では typed `SourceRef<T>` / `SinkRef<T>` payload workflow を主経路にし、actor path string を主 API にしない。

## 3. Endpoint Actor Ownership And Wake Contract

- [x] 3.1 producer stream から materialized `SourceRef<T>` を作るとき、どの endpoint actor がどの stream resource に所有されるかを定義する。
- [x] 3.2 consumer sink から materialized `SinkRef<T>` を作るとき、どの endpoint actor がどの stream resource に所有されるかを定義する。
- [x] 3.3 handshake、demand、element、completion、failure、cancellation が endpoint state を更新した後、materialized stream を wake / drive する経路を実装または明示する。
- [x] 3.4 completion / cancellation / failure 時に endpoint actor が deterministic shutdown し、watch release failure / shutdown failure を観測可能にする。
- [x] 3.5 one-shot partner pairing、double materialization、non-partner message の observable failure を endpoint actor state に組み込む。

## 4. Local Actor-Backed Resolver Gate

- [x] 4.1 local handoff だけでなく actor-backed endpoint を持つ `SourceRef<T>` を materialize し、canonical actor path string に変換できることを lower-level test で確認する。
- [x] 4.2 actor-backed `SinkRef<T>` を materialize し、canonical actor path string に変換できることを lower-level test で確認する。
- [x] 4.3 serialized `SourceRef` は `SourceRef<T>` へ、serialized `SinkRef` は `SinkRef<T>` へ ActorSystem provider dispatch 経由で resolve する。
- [x] 4.4 loopback authority は local actor delivery に解決され、transport connection を直接組み立てないことを確認する。
- [x] 4.5 unsupported local-only ref / missing endpoint actor / invalid path format は成功にせず、明示 failure として返す。

## 5. Protocol Serializer And Payload Support

- [x] 5.1 StreamRef protocol payload serializer または manifest route を登録する。
- [x] 5.2 typed `SourceRef<T>` / `SinkRef<T>` を domain message payload として扱える serializer support を追加する。
- [x] 5.3 missing serializer registration、unsupported payload manifest、type mismatch の failure tests を追加する。
- [x] 5.4 StreamRef protocol message は通常の remote actor envelope に載せ、`RemoteTransport` に StreamRef 専用 method / wire frame を追加しない。

## 6. Backpressure And Terminal Semantics

- [x] 6.1 cumulative stream-level demand なしに element が配送されないことを検証する。
- [x] 6.2 demand 到着前または transport enqueue backpressure 中の accepted element を silently drop しないことを検証する。
- [x] 6.3 pending sequenced elements が配送された後にのみ normal completion が観測されることを検証する。
- [x] 6.4 failure / cancellation / invalid sequence / invalid demand / invalid partner / duplicate materialization が normal completion に潰されないことを検証する。
- [x] 6.5 cancellation が remote partner へ伝播し、その ref への追加 publication を止めることを検証する。

## 7. Remote Integration

- [x] 7.1 typed `SourceRef<T>` を remote message payload として渡し、two-ActorSystem 間で backpressure 付き stream が流れる integration test を追加する。
- [x] 7.2 typed `SinkRef<T>` を remote message payload として渡し、two-ActorSystem 間で backpressure 付き stream が流れる integration test を追加する。
- [x] 7.3 partner DeathWatch、address termination、transport connection loss が protocol completion 前に stream failure として観測される remote failure integration tests を追加する。
- [x] 7.4 remote watch hook と endpoint partner watch / unwatch を接続し、watch release failure を握りつぶさない。

## 8. Documentation And Verification

- [x] 8.1 local actor-backed resolver proof と two-ActorSystem typed payload proof が通った後にのみ `docs/gap-analysis/stream-gap-analysis.md` を更新する。
- [x] 8.2 targeted crate tests、`mise exec -- openspec validate add-remote-stream-ref-transport --strict`、`git diff --check` を実行する。
- [x] 8.3 明示的に狭い検証範囲が承認されていない限り、完了前に `./scripts/ci-check.sh ai all` を実行する。

### Verification Gate

PR 可能条件は、fake local format ではなく actor-backed endpoint ref の local resolver proof と、two-ActorSystem typed `SourceRef` / `SinkRef` payload tests が `#[ignore]` なしで pass することである。
