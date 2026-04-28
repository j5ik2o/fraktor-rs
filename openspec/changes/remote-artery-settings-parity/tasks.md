## 1. 既存パターン確認

- [x] 1.1 `RemoteConfig` の既存 builder / accessor / validation / test のパターンを確認する
- [x] 1.2 `association_runtime` の outbound restart 実装、`RestartCounter`、`ReconnectBackoffPolicy` の責務分離を確認する
- [x] 1.3 `core/wire` と `tcp_transport` が fraktor-rs 独自 wire format を維持していることを確認し、Pekko byte compatibility を実装対象に含めない境界を確認する

## 2. remote-core settings

- [x] 2.1 large-message destinations を no_std core で所有できる型として追加し、HOCON parser や regex dependency を導入しない
- [x] 2.2 `RemoteConfig` に outbound large-message queue size と large-message destinations のフィールド、デフォルト値、builder、accessor を追加する
- [x] 2.3 outbound large-message queue size が `0` の場合に拒否される validation を追加する
- [x] 2.4 `RemoteConfig` に inbound restart timeout と inbound max restarts のフィールド、デフォルト値、builder、accessor を追加する
- [x] 2.5 `RemoteConfig` に compression settings の設定 surface を追加し、wire-level compression behavior は追加しない
- [x] 2.6 `modules/remote-core/src/core/config/tests.rs` に defaults、builder chain、validation、immutability、no_std 境界のテストを追加する

## 3. remote-adaptor-std runtime

- [x] 3.1 std adaptor が `RemoteConfig` から inbound restart timeout と inbound max restarts を association runtime に渡す経路を追加する
- [x] 3.2 inbound loop の restart に既存 `RestartCounter` と同じ deadline-window budget 意味論を適用する
- [x] 3.3 inbound restart budget 超過時に無限 restart せず、観測可能な error path に失敗を返す
- [x] 3.4 restart window の判定が `Instant` ベースの monotonic millis で行われ、`SystemTime` に依存しないことを確認する
- [x] 3.5 large-message / compression settings を参照可能にしても、Pekko Artery TCP framing、protobuf control PDU、compression table の wire 互換処理を追加しない
- [x] 3.6 `modules/remote-adaptor-std/src/std/association_runtime/tests.rs` に inbound restart budget、budget reset、budget exhaustion、wire codec 非変更のテストを追加する

## 4. 検証

- [x] 4.1 `cargo test -p fraktor-remote-core-rs core::config` を実行して remote-core settings のテストを確認する
- [x] 4.2 `cargo test -p fraktor-remote-adaptor-std-rs association_runtime` を実行して std runtime のテストを確認する
- [x] 4.3 `openspec validate remote-artery-settings-parity --strict` を実行して OpenSpec delta を検証する
- [x] 4.4 ソースコードを編集した implementation phase の最後に `./scripts/ci-check.sh ai all` を実行し、エラーがないことを確認する
