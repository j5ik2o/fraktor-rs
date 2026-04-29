# remote Phase 1B std runtime lifecycle 実装計画

## 目的
remote Phase 1B として、std adapter に残っている advertised addresses / listen event と association lifecycle event publishing の配線を実装する。

## 実装範囲
- `StdRemoting` が transport の advertised addresses を snapshot として保持し、`addresses(&self)` で返す。
- `StdRemoting::start` が transport start 成功後に `ListenStarted` を actor-core event stream へ発行する。
- `RemotingExtensionInstaller::install` が `EventPublisher` を生成して `StdRemoting` に注入する。
- `AssociationEffect::PublishLifecycle` を tracing だけで終わらせず、`EventPublisher::publish_lifecycle` で event stream へ配送する。
- `HandshakeDriver` と `run_inbound_dispatch` の呼び出し経路へ `EventPublisher` を明示的に渡す。

## 非対象
- 実 listener bind を同期 `start()` に統合する Phase 2 タスク
- handshake validation / retry / liveness probe
- per-peer inbound routing
- payload serialization / inbound envelope delivery
- Pekko Artery TCP framing
- remote actor ref 生成 / send path / DeathWatch

## 検証
- `cargo fmt --all`
- 変更範囲の型チェック / clippy
- `cargo test -p fraktor-remote-adaptor-std-rs extension_installer`
- `cargo test -p fraktor-remote-adaptor-std-rs association`
- `./scripts/ci-check.sh ai dylint`
