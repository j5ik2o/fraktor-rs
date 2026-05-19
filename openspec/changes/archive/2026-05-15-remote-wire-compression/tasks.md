## 1. ベースラインとスコープ確認

- [x] 1.1 `remote-payload-serialization`、`remote-reliable-deathwatch`、`remote-graceful-flush` の実装状態を current code と照合する。
- [x] 1.2 編集前に既存の `RemoteCompressionConfig`、`EnvelopePdu`、`ControlPdu`、`TcpRemoteTransport` の責務境界を確認する。
- [x] 1.3 affected remote crates の targeted test と coverage baseline を記録し、patch coverage が低下しないようにする。

Baseline evidence (2026-05-15): `cargo test -p fraktor-remote-core-rs wire --lib`, `cargo test -p fraktor-remote-core-rs config --lib`, and `cargo test -p fraktor-remote-adaptor-std-rs transport::tcp --lib` passed. `cargo llvm-cov -p fraktor-remote-core-rs -p fraktor-remote-adaptor-std-rs --lib --summary-only` reported total region coverage 95.43% and line coverage 95.39%.

## 2. remote-core compression table 実装

- [x] 2.1 actor-ref と manifest の entry、entry id、generation、advertisement、acknowledgement、hit count を扱う no_std compression table 型を追加する。
- [x] 2.2 `RemoteCompressionConfig` の max が `None` の場合の kind 単位 disabled behavior を実装する。
- [x] 2.3 configured max で bounded された deterministic advertisement candidate selection を実装する。
- [x] 2.4 hit counting、disabled-kind no-op、advertisement generation、ack application、stale ack ignore、literal fallback の unit tests を追加する。

## 3. wire format 実装

- [x] 3.1 `CompressedText` の literal / reference metadata と、valid literal、valid reference、unknown tag、truncation の codec coverage を追加する。
- [x] 3.2 recipient path、sender path、manifest が serializer id や payload bytes を圧縮せずに `CompressedText` を保持できるよう `EnvelopePdu` wire metadata を拡張する。
- [x] 3.3 `ControlPdu` と `ControlCodec` に `CompressionAdvertisement` と `CompressionAck` subkind を追加する。
- [x] 3.4 compressed envelope metadata と compression control PDU の round-trip / invalid-input tests を追加する。

## 4. std TCP Transport 適用

- [x] 4.1 `RemoteConfig::compression_config()` から peer-local compression tables と advertisement timer configuration を初期化する。
- [x] 4.2 outbound `OutboundEnvelope` serialization から `EnvelopePdu` へ変換する際に、ack 済み actor-ref / manifest table references を適用する。
- [x] 4.3 missing または unacked の table entry では literal fallback を維持し、compression miss だけで send を失敗させない。
- [x] 4.4 inbound compressed envelope metadata を `RemoteEvent::InboundFrameReceived` 送信前に literal `EnvelopePdu` へ復元する。
- [x] 4.5 inbound compression advertisement / ack control frames は transport 内で消費し、core event loop へ転送しない。
- [x] 4.6 compression acknowledgement を既存 peer writer path 経由で送信し、invalid compression control failure を tests または logs で観測可能にする。

## 5. integration tests と coverage

- [x] 5.1 advertisement が inbound tables を更新し、matching ack を送信することを示す transport-level tests を追加する。
- [x] 5.2 acked metadata が outbound で compression され、unknown inbound references が拒否されることを示す transport-level tests を追加する。
- [x] 5.3 config-driven disabled actor-ref / manifest compression の tests を追加する。
- [x] 5.4 affected targeted tests を実行し、project / patch coverage が低下していないことを確認する。

Coverage evidence (2026-05-15): `cargo test -p fraktor-remote-core-rs wire --lib`, `cargo test -p fraktor-remote-core-rs config --lib`, and `cargo test -p fraktor-remote-adaptor-std-rs transport::tcp --lib` passed after implementation. `cargo llvm-cov -p fraktor-remote-core-rs -p fraktor-remote-adaptor-std-rs --lib --summary-only` reported total region coverage 95.43% and line coverage 95.52%, preserving the pre-change 95.43% region baseline and improving the 95.39% line baseline.

## 6. ドキュメントと検証

- [x] 6.1 `docs/gap-analysis/remote-gap-analysis.md` を更新し、wire compression table application が完了または部分完了したことと残 gap を記録する。
- [x] 6.2 `mise exec -- openspec validate remote-wire-compression --strict` を実行する。
- [x] 6.3 `git diff --check` を実行する。
