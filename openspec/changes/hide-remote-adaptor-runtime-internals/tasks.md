## 1. 公開 API の現状固定

- [x] 1.1 `remote-adaptor-std` の `pub mod` / `pub use` / `pub fn` を棚卸しし、利用者向け境界・adapter bridge・runtime internal・低レベル provider plumbing に分類する。
- [x] 1.2 外部 crate から import できてよい型を `TcpRemoteTransport`、`RemotingExtensionInstaller`、高レベル provider installer / config API に絞る public surface test を追加または更新する。
- [x] 1.3 runtime internal として閉じる型・関数（`TcpClient` / `TcpServer` / `WireFrameCodec` / `InboundFrameEvent` / `run_inbound_dispatch` / `WatcherActor` / `RemoteActorRefSender` / `TokioMpscRemoteEventReceiver` 等）が external crate から import できないことを固定する。

## 2. remote lifecycle control の内部化

- [x] 2.1 `RemotingExtensionInstaller::install` または ActorSystem lifecycle hook が、install 後に core の `RemoteShared::start()` または同等 lifecycle operation を内部で呼ぶようにする。
- [x] 2.2 `RemotingExtensionInstaller::install` または ActorSystem lifecycle hook が、`TokioMpscRemoteEventReceiver` を使う run task を内部で起動するようにする。
- [x] 2.3 通常利用者向け API / showcase から `installer.remote()?.start()` と `installer.spawn_run_task()` を削除する。
- [x] 2.4 ActorSystem termination から core shutdown semantics、event loop wake、tokio `JoinHandle` の完了観測へ到達する経路を追加する。
- [x] 2.5 通常利用者向け API / showcase から `installer.shutdown_and_join().await` を削除する。
- [x] 2.6 `installer.remote()` が残る場合は診断・内部テスト用途に限定し、startup API として docs / showcase / public surface test に出ないことを固定する。

## 3. TCP transport 公開面の縮小

- [x] 3.1 `transport::tcp` の公開 re-export を `TcpRemoteTransport` 中心に整理し、`TcpClient` / `TcpServer` / frame codec 系型を crate 内部へ移す。
- [x] 3.2 `TcpRemoteTransport` の public method signature から内部 TCP 実装型や runtime channel 型を漏らさない。
- [x] 3.3 crate 内 runtime が必要とする transport 補助 API は `pub(crate)` に限定し、外部利用者向け API と分離する。
- [x] 3.4 `connect_peer` / `send_handshake` / `send_control` / `schedule_handshake_timeout` の公開範囲を、core `RemoteTransport` port と runtime internal のどちらに置くか確認し、public inherent method として不要なものは閉じる。

## 4. runtime driver の内部化

- [x] 4.1 `std::association` module と `run_inbound_dispatch` を crate 内部 API に変更する。
- [x] 4.2 `watcher_actor` と heartbeat 関連型を crate 内部 API に変更する。
- [x] 4.3 `RemoteActorRefSender` と送信エラー型を crate 内部 API に変更し、外部利用者に runtime 実装型を露出しない。
- [x] 4.4 `TokioMpscRemoteEventReceiver` を crate 内部 API に変更し、remote event receiver 実装を user-facing public surface から外す。
- [x] 4.5 内部 API に依存しているテストは crate 内部テストへ移し、外部 integration test は public API のみを使う。

## 5. provider 配線の隠蔽

- [x] 5.1 `StdRemoteActorRefProvider::new` の公開範囲を見直し、外部 crate から低レベル依存を渡して構築できないようにする。
- [x] 5.2 `local_provider` / `remote_provider` / `event_sender` / `resolve_cache` / `event_publisher` / monotonic epoch の組み立てを installer/config 側へ移す。
- [x] 5.3 `PathRemoteActorRefProvider` または同等の low-level remote-only provider を通常利用者に直接注入させない高レベル provider installer / config API を用意する。
- [x] 5.4 `StdRemoteActorRefProvider` の低レベル accessor が public API として必要か見直し、不要なら非公開化する。
- [x] 5.5 actor-core の `ActorRefProvider` と remote-core の `RemoteActorRefProvider` をつなぐ責務は維持し、core wrapper にならないことを確認する。

## 6. routee / showcase の更新

- [x] 6.1 `RemoteRouteeExpansion` が手動 provider 配線を要求する場合は installer/config 経由に変更する。
- [x] 6.2 installer/config 経由に移せない低レベル routee helper は public surface から外す。
- [x] 6.3 `showcases/std/legacy/remote_lifecycle/main.rs` から runtime internal の直接 import と direct lifecycle call を削除する。
- [x] 6.4 remote lifecycle showcase が `TcpRemoteTransport` / `RemotingExtensionInstaller` / `ActorSystemConfig::with_extension_installers` だけで remote を有効化することを示すように更新する。
- [x] 6.5 lifecycle event を ActorSystem 作成後に subscribe して検証している場合は、remote 自動開始による event 取り逃がしを避ける test / showcase に置き換える。

## 7. 検証

- [x] 7.1 `rtk cargo test -p fraktor-remote-core-rs` を実行する。
- [x] 7.2 `rtk cargo test -p fraktor-remote-adaptor-std-rs` を実行する。
- [x] 7.3 `rtk cargo test -p fraktor-cluster-adaptor-std-rs` を実行する。
- [x] 7.4 ソースコード編集後の最終確認として `rtk ./scripts/ci-check.sh ai all` を完了まで実行する。
