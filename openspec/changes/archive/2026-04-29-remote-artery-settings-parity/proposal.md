## Why

`remote` の Pekko parity は基礎部品が揃っている一方で、`RemoteConfig` と std runtime がまだ一部の Artery advanced settings を表現できない。特に large-message と inbound restart の設定境界が未定義なため、後続の remote delivery / compression 実装時に設定項目を場当たり的に追加するリスクがある。

## What Changes

- `RemoteConfig` に、Pekko Artery advanced settings のうち L1 responsibility parity として必要な設定 surface を追加する。
- large-message routing の設定は実際の large-message transport / compression 実装までは行わず、宛先判定と queue limit を型付き設定として保持できる範囲に限定する。
- inbound restart budget は std runtime が参照可能な設定として定義し、outbound restart と対になる runtime contract を明確にする。
- compression settings は wire-level compression protocol 実装には踏み込まず、後続 Phase3 の入力となる設定 surface として扱う。
- `ActorIdentity` remote ActorRef restoration と `RemoteRouterConfig` runtime routee expansion はこの change に含めない。どちらも concrete remote `ActorRef` construction に依存し、別 change で扱う。

## Capabilities

### New Capabilities

なし。

### Modified Capabilities

- `remote-core-settings`: `RemoteConfig` が Artery advanced settings parity に必要な large-message / inbound restart / compression 設定を型付き builder + accessor として保持する。
- `remote-adaptor-std-runtime`: std runtime が inbound restart 設定を参照でき、設定値の適用境界を actor delivery / wire compression 実装から分離する。

## Impact

- 影響コード:
  - `modules/remote-core/src/core/config/remote_config.rs`
  - `modules/remote-core/src/core/config/tests.rs`
  - `modules/remote-adaptor-std/src/std/association_runtime/`
  - `modules/remote-adaptor-std/src/std/association_runtime/tests.rs`
- 影響spec:
  - `openspec/specs/remote-core-settings/spec.md`
  - `openspec/specs/remote-adaptor-std-runtime/spec.md`
- 破壊的変更:
  - 正式リリース前のため後方互換は維持しない。ただし既存 builder / accessor は原則維持し、新規設定を追加する形に留める。
- 非対象:
  - Pekko Artery wire compatibility
  - payload serialization
  - concrete remote `ActorRef` construction
  - `ActorIdentity` remote ActorRef restoration
  - `RemoteRouterConfig` routee materialization
