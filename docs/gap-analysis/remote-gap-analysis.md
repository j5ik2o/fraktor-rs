# remote モジュール ギャップ分析

> 分析日: 2026-02-28
> 対象: `modules/remote/src/` vs `references/pekko/remote/src/main/`

## サマリー

| 指標 | 値 |
|---|---:|
| Pekko 公開型数 | 299 |
| fraktor-rs 公開型数 | 97 |
| 同名型カバレッジ | 10/299 (3.3%) |
| ギャップ数（同名差分） | 289 |

> 注: JVM/Artery 固有 API が多く、Rust/no_std 方針に合わせて対象外とする項目を含む。

## 主要ギャップ

| Pekko API | fraktor対応 | 難易度 | 判定 |
|---|---|---|---|
| Artery/Aeron 輸送層 | `RemoteTransport` 抽象のみ | hard / n/a | 未実装（基盤のみ） |
| TLS SSLEngine Provider | 未対応 | n/a | JVM依存で対象外候補 |
| AckedSendBuffer / AckedReceiveBuffer | `AckedDelivery` | medium | 部分実装 |
| quarantine セマンティクス | `quarantine(authority, reason)` 実装あり | medium | 部分実装 |
| HandshakeReq / HandshakeRsp | `HandshakeFrame` + `HandshakeKind` | - | 別名で実装済み |

## 根拠（主要参照）

- Pekko:
  - `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/ArterySettings.scala:35`
  - `references/pekko/remote/src/main/scala/org/apache/pekko/remote/transport/netty/SSLEngineProvider.scala:43`
  - `references/pekko/remote/src/main/scala/org/apache/pekko/remote/AckedDelivery.scala:111`
- fraktor-rs:
  - `modules/remote/src/core/transport/remote_transport.rs:20`
  - `modules/remote/src/core/envelope/acked_delivery.rs:22`
  - `modules/remote/src/core/remoting_extension/control_handle.rs:281`
  - `modules/remote/src/core/handshake/frame.rs:10`
  - `modules/remote/src/core/handshake/kind.rs:5`

## 実装優先度提案

1. Phase 1 (medium): quarantine の状態遷移と理由反映を強化
2. Phase 2 (medium): ack バッファ戦略（送受信ウィンドウ）を追加
3. Phase 3 (hard): Artery/Aeron 相当の輸送層拡張（必要時のみ）
