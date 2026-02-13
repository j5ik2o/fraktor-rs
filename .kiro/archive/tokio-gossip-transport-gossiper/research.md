# 調査ログ: tokio-gossip-transport-gossiper

## Summary
- `GossipTransport` は同期/ポーリング API のため、Tokio の非同期 I/O はチャネルでブリッジする設計が適合する。
- `tokio::net::UdpSocket` は datagram のサイズ不足で破棄が起き得るため、最大サイズを明示してバッファを固定する必要がある。
- `tokio::time::interval` は一定周期の tick を供給でき、Gossiper の周期処理に使える。

## Discovery Scope
- WebSearch: Tokio の UDP、mpsc、interval の公式ドキュメント。
- Codebase: `modules/cluster/src/core` の `GossipTransport`/`MembershipCoordinator`/`GossipEngine`、`modules/cluster/src/std/membership_coordinator_driver.rs`。

## Research Log

### 1. Tokio UdpSocket の受信・バッファ特性
**Findings**
- `UdpSocket::recv_from`/`try_recv` は受信バッファが不足すると超過分を破棄するため、最大サイズを明示したバッファ確保が必要。
- UDP の送信元アドレスは信頼できないため、アプリケーション層での扱いに注意が必要。

**Sources**
- https://docs.rs/tokio/latest/tokio/net/struct.UdpSocket.html

**Implications**
- `TokioGossipTransportConfig` に `max_datagram_bytes` を持たせ、受信バッファを固定化する。
- 送信元 authority を無条件で信頼せず、必要なら許可リスト/ブロックリストで絞り込める設計を残す。

### 2. Tokio mpsc の同期/非同期ブリッジ
**Findings**
- mpsc の bounded チャネルは `try_send` により即時失敗を返せる。
- `Receiver::try_recv` は空/切断を区別でき、ポーリング実装に適用できる。

**Sources**
- https://docs.rs/tokio/latest/tokio/sync/mpsc/
- https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Sender.html
- https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Receiver.html

**Implications**
- `GossipTransport::send` は outbound キューに `try_send` し、失敗時は `GossipTransportError::SendFailed` を返す。
- `poll_deltas` は `try_recv` を drain して Vec を返す。

### 3. Tokio interval の周期処理
**Findings**
- `tokio::time::interval` は固定周期で tick を供給し、`Interval` を drop すると停止する。
- 期間が 0 の場合は panic するため、最小周期を構成で保証する必要がある。

**Sources**
- https://docs.rs/tokio/latest/tokio/time/fn.interval.html

**Implications**
- `TokioGossiperConfig` に `tick_interval` を持たせ、0 の指定を拒否する。
- `TokioGossiper::stop` は interval を drop し、タスク停止を保証する。

## Architecture Pattern Evaluation
- **Option A (既存 driver 拡張)**: 既存 `MembershipCoordinatorDriverGeneric` を中心に transport と gossiper を追加する。
- **Option B (新規分離)**: transport/gossiper を独立レイヤとして新設する。
- **Option C (段階導入)**: 最小 UDP transport を先に導入し、後から拡張する。

**Decision**: Option A を基本にし、wire 仕様や容量制限は Config で拡張可能にする。

## Design Decisions
- `TokioGossipTransport` は UDP + mpsc で同期 API を満たす。
- `TokioGossiper` は interval で `MembershipCoordinatorDriverGeneric` を周期駆動する。
- wire 形式は std 側の専用 struct を serde + postcard でエンコードする。

## Risks and Mitigations
- **UDP 偽装/破棄**: 送信元偽装と datagram 破棄が起き得る → 受信サイズ制限とフィルタ拡張ポイントを用意。
- **同期 API と非同期 I/O の間の背圧**: outbound キュー溢れ → `try_send` 失敗をエラー化。

## Open Questions
- gossip wire の将来互換（protoactor-go 互換）をいつ導入するか。
