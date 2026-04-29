## 1. 公開 API の現状固定

- [ ] 1.1 `remote-adaptor-std` の `pub mod` / `pub use` / `pub fn` を棚卸しし、利用者向け境界・adapter bridge・runtime internal に分類する。
- [ ] 1.2 外部 crate から import できてよい型を `TcpRemoteTransport` と installer/config 系 API に絞る public surface test を追加または更新する。
- [ ] 1.3 `StdRemoteActorRefProvider` は adapter bridge として残しつつ、利用者が低レベル依存を手動注入しないことをテストで固定する。

## 2. TCP transport 公開面の縮小

- [ ] 2.1 `tcp_transport` の公開 re-export を `TcpRemoteTransport` 中心に整理し、`TcpClient` / `TcpServer` / frame codec 系型を crate 内部へ移す。
- [ ] 2.2 `TcpRemoteTransport` の public method signature から内部 TCP 実装型や runtime channel 型を漏らさない。
- [ ] 2.3 crate 内 runtime が必要とする transport 補助 API は `pub(crate)` に限定し、外部利用者向け API と分離する。

## 3. runtime driver の内部化

- [ ] 3.1 `association_runtime` の registry / shared / handshake / reconnect / quarantine / inbound / outbound driver を crate 内部 API に変更する。
- [ ] 3.2 `watcher_actor` と heartbeat 関連型を crate 内部 API に変更する。
- [ ] 3.3 `RemoteActorRefSender` と送信エラー型を crate 内部 API に変更し、外部利用者に runtime 実装型を露出しない。
- [ ] 3.4 内部 API に依存しているテストは crate 内部テストへ移し、外部 integration test は public API のみを使う。

## 4. provider 配線の隠蔽

- [ ] 4.1 `StdRemoteActorRefProvider::new` の公開範囲を見直し、外部 crate から低レベル依存を渡して構築できないようにする。
- [ ] 4.2 `local_provider` / `remote_provider` / `transport` / `resolve_cache` / `event_publisher` の組み立てを installer/config 側へ移す。
- [ ] 4.3 `StdRemoteActorRefProvider` の `transport()` など低レベル依存を返す accessor が public API として必要か見直し、不要なら非公開化する。
- [ ] 4.4 actor-core の `ActorRefProvider` と remote-core の `RemoteActorRefProvider` をつなぐ責務は維持し、core wrapper にならないことを確認する。

## 5. routee / showcase の更新

- [ ] 5.1 `RemoteRouteeExpansion` が手動 provider 配線を要求する場合は installer/config 経由に変更する。
- [ ] 5.2 installer/config 経由に移せない低レベル routee helper は public surface から外す。
- [ ] 5.3 `showcases/std/remote_lifecycle` と `showcases/std/remote_routee_expansion` から runtime internal の直接 import を削除する。
- [ ] 5.4 showcase が `remote-core::Remote` と `remote-adaptor-std` の adapter 境界だけで利用できることを確認する。

## 6. 検証

- [ ] 6.1 `rtk cargo test -p fraktor-remote-core-rs` を実行する。
- [ ] 6.2 `rtk cargo test -p fraktor-remote-adaptor-std-rs` を実行する。
- [ ] 6.3 `rtk cargo test -p fraktor-cluster-adaptor-std-rs` を実行する。
- [ ] 6.4 ソースコード編集後の最終確認として `rtk ./scripts/ci-check.sh ai all` を完了まで実行する。
