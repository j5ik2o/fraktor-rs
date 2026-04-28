# remote outbound maximum_frame_size 実装計画

## 対象

Phase 1 の `outbound maximum_frame_size enforcement` のみを実装する。

Phase 2 の `ActorIdentity` remote ActorRef restoration、`RemoteRouterConfig` runtime routee expansion、advanced Artery settings 残りは、provider / routing / runtime queue への波及が大きいため今回の変更には含めない。

## 変更予定

| 種別 | ファイル |
|------|---------|
| 変更 | `modules/remote-adaptor-std/src/std/tcp_transport/frame_codec.rs` |
| 変更 | `modules/remote-adaptor-std/src/std/tcp_transport/tests.rs` |

## 実装方針

`WireFrameCodec::encode` は直接 `dst` に書き込まず、一時 `BytesMut` に core PDU codec の出力を書き込む。encode 後、現行 wire format の length prefix が表す `version + kind + body` の長さを `maximum_frame_size` と比較する。

上限を超えた場合は既存の `WireError::FrameTooLarge` を返し、`dst` は変更しない。上限ちょうどの場合は許可する。

## スコープ外

- Pekko Artery TCP framing の wire 互換化
- message payload serialization into envelope
- inbound envelope delivery
- remote ActorRef 実体化
- DeathWatch / watcher effects application
