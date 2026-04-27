# RemoteTransport.start 実 bind 契約 実装計画

## 背景

TAKT `pekko-porting` ワークフローの implement ステップとして、Report Directory の `00-plan.md` に従い、remote Phase 2 の `RemoteTransport.start` 実 bind 契約だけを実装する。

## 実装対象

- `TcpServer::start` を同期関数に変更し、現在の Tokio runtime 上で listener を bind して accept loop を spawn する。
- `TcpRemoteTransport::start` から `TcpServer::start` を呼び、bind 成功後だけ running 状態にする。
- bind port `0` の advertised address を実 bound port に更新する。
- `StdRemoting::start` と extension installer 経路の既存到達経路は維持する。

## 非対象

- inbound envelope delivery
- Pekko Artery TCP framing 完全互換
- payload serialization
- handshake validation / retry / liveness probe
- remote actor ref construction
- `./scripts/ci-check.sh ai all`

## 検証

- `rtk cargo clippy -p fraktor-remote-adaptor-std-rs -- -D warnings`
- `rtk cargo test -p fraktor-remote-adaptor-std-rs`
- `rtk ./scripts/ci-check.sh ai dylint`
