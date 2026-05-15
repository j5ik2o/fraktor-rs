## 背景

`RemoteScope` と `RemoteRouterConfig` は remote deployment の配置先を表現できるが、runtime にはその deployment descriptor を target node 上の live child actor に変換する remote create protocol と std daemon がまだ存在しない。

この change では、既存の remote serialization、DeathWatch、flush、compression の土台を actor 作成へ接続し、caller が remote child を spawn して返却された remote ref へ message を送れるようにする。

## 変更内容

- child spawn 時に `Scope::Remote` を持つ `Deploy` を actor-core が検出し、local actor cell を作る代わりに remote deployment hook へ委譲する。
- remote actor create request、create success、create failure を表す fraktor-native wire PDU を追加する。
- std remote deployment daemon を追加し、create request の受信、deployment contract の検証、target node 上での actor spawn、created actor path の応答を行う。
- origin node 側 provider に request tracking を追加し、create acknowledgement を待ってから返却 path を `StdRemoteActorRefProvider` 経由で remote ref に解決し、spawn failure を観測可能にする。
- deployable actor factory id と deployment payload は actor-core serialization を使う。任意の Rust closure や raw `Props` 内部表現は serialize しない。
- Pekko Artery byte compatibility、cluster placement、security/authentication policy、generic closure shipping は対象外とする。

## Capabilities

### New Capabilities

- `actor-core-remote-deployment`: actor-core の `RemoteScope` spawn dispatch、deployable props contract、remote deployment の spawn error semantics を定義する。
- `remote-deployment-daemon`: std remote deployment daemon の request tracking、target-node actor creation、response handling、two-node behavior を定義する。

### Modified Capabilities

- `remote-core-wire-format`: fraktor-native wire format に remote deployment create request/response PDU を追加する。
- `remote-adaptor-std-extension-installer`: deployment daemon を既存 std remoting lifecycle と一緒に start / stop する。
- `remote-adaptor-std-provider-dispatch`: installed std remote actor-ref provider 経由で remote deployment path を公開し、loopback dispatch は local のまま維持する。

## 影響範囲

- `modules/actor-core-kernel/src/actor/deploy/`
- `modules/actor-core-kernel/src/actor/props/`
- `modules/actor-core-kernel/src/actor/spawn/`
- `modules/remote-core/src/wire/`
- `modules/remote-adaptor-std/src/extension_installer/`
- `modules/remote-adaptor-std/src/provider/`
- `modules/remote-adaptor-std/src/deployment/`
- `modules/remote-adaptor-std/tests/`
- `docs/gap-analysis/remote-gap-analysis.md`

これは remote deployment の振る舞いを追加する change であり、public compatibility layer ではない。`remote-core` は no_std を維持し、std task / timer / channel / local actor creation は `remote-adaptor-std` と `actor-core-kernel` に閉じる。
