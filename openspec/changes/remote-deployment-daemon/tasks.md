## 1. ベースラインと契約確認

- [x] 1.1 payload serialization、reliable DeathWatch、graceful flush、wire compression の remote 前提が現行コードに存在することを確認する。
- [x] 1.2 編集前に既存の `Deploy`、`RemoteScope`、`Props`、`SpawnError`、`StdRemoteActorRefProvider`、`RemotingExtensionInstaller` の責務境界を確認する。
- [x] 1.3 actor-core deploy/spawn、remote-core wire、remote-adaptor-std provider、two-node remote tests の targeted baseline を記録する。

Baseline evidence (2026-05-15): `cargo test -p fraktor-actor-core-kernel-rs deploy --lib` passed 7 tests, `cargo test -p fraktor-actor-core-kernel-rs spawn --lib` passed 28 tests, `cargo test -p fraktor-remote-core-rs wire --lib` passed 88 tests, `cargo test -p fraktor-remote-adaptor-std-rs provider --lib` passed 37 tests, and `cargo test -p fraktor-remote-adaptor-std-rs --test two_node_actor_system_delivery` passed 6 tests.

## 2. actor-core Remote Deployment Surface

- [x] 2.1 stable factory id と remote-serializable factory payload を持つ deployable props metadata を追加する。
- [x] 2.2 raw Rust closure を wire 越しに公開せず、deployable factory registry または同等の target-node lookup contract を追加する。
- [x] 2.3 actor-core remote deployment hook の registration と、`Scope::Remote` child spawn からの invocation、および `RemoteCreated` / `UseLocalDeployment` / `Failed` 相当の outcome handling を追加する。
- [x] 2.4 missing hook、non-deployable props、remote create failure、timeout を observable `SpawnError` に map する。
- [x] 2.5 remote-scoped spawn が local actor cell を作らず、failure 時に local fallback しないことを unit tests で保証する。
- [x] 2.6 remote child の `ChildRef::stop` / `suspend` / `resume` 相当が local lifecycle command として誤処理されず、unsupported failure として観測できることを保証する。

## 3. remote-core Wire Protocol

- [x] 3.1 correlation id と structured failure code を持つ `RemoteDeploymentPdu` request/success/failure data type を追加する。
- [x] 3.2 target parent path、child name、factory id、serializer id、manifest、payload bytes を含む create request payload metadata の codec support を追加する。
- [x] 3.3 deployment frame が user `EnvelopePdu` delivery と区別できることを保証する。
- [x] 3.4 新しい wire type に対し、round-trip、truncation、invalid tag、invalid payload metadata、no_std build coverage を追加する。

## 4. std Deployment Daemon

- [x] 4.1 daemon command handling と task ownership を持つ `remote-adaptor-std` deployment module を追加する。
- [x] 4.2 public manual lifecycle API を追加せず、daemon startup/shutdown を `RemotingExtensionInstaller` に接続する。
- [x] 4.3 create request payload は daemon-local registry ではなく actor system serialization extension で deserialize する。
- [x] 4.4 target node 上で deployable factory id を解決し、要求された target parent path と child name の下に actor を作成する。
- [x] 4.5 create success では canonical remote path を、create failure では structured error と reason を返す。
- [x] 4.6 inbound deployment request frame は daemon request handler へ、deployment response frame は provider pending response handler へ routing する。

## 5. Provider Integration

- [x] 5.1 `StdRemoteActorRefProviderInstaller` から actor-core remote deployment hook を登録する。
- [x] 5.2 correlation id を key にした origin-side pending request state と bounded synchronous timeout handling を追加する。
- [x] 5.3 successful create response を既存 `StdRemoteActorRefProvider` path resolution 経由で remote ref に変換する。
- [x] 5.4 stale または unknown create response を log または test-observable error path で拒否する。
- [x] 5.5 local node を指す remote scope は hook outcome で actor-core の local spawn path に戻し、既存 loopback behavior を維持する。
- [x] 5.6 bounded wait が remote run task、TCP reader task、deployment response dispatcher task を block しないことを tests で保証する。

## 6. Integration Tests と Documentation

- [x] 6.1 deployable actor を remote-spawn し、返却 remote ref へ user message を送る two-node test を追加する。
- [x] 6.2 unknown factory id、duplicate child name、timeout、non-deployable props の failure-path tests を追加する。
- [ ] 6.3 parent が remote child termination を既存 remote DeathWatch path で観測する coverage を追加する。
- [x] 6.4 実装後に `docs/gap-analysis/remote-gap-analysis.md` を更新する。
- [x] 6.5 affected crates の targeted tests を実行し、その後 `mise exec -- openspec validate remote-deployment-daemon --strict` と `git diff --check` を実行する。
