# SystemMessageDelivery 再送・nack 実装計画

## 背景

remote Phase 2 medium 項目のうち、今回の対象は `system message delivery retransmission / nack` に限定する。Pekko Artery の system message delivery は、未 ack の system message を保持し、`ResendTick` で再送し、`Ack` / `Nack` を受けて累積 ack を処理する。

## 対象

- `modules/remote-adaptor-std/src/std/association_runtime/system_message_delivery.rs`
- `modules/remote-adaptor-std/src/std/association_runtime/tests.rs`

## 実装方針

1. `SystemMessageDeliveryState` の pending entry に sequence / envelope / last_sent_at_ms を保持する。
2. `record_send(envelope, now_ms)` で送信時刻を記録する。
3. `due_retransmissions(now_ms, resend_interval_ms)` を query として追加する。
4. `mark_retransmitted(sequence_number, now_ms)` を command として追加する。
5. `nacked_pending(&AckPdu)` で nack bitmap から pending 内の sequence だけを返す。
6. `apply_ack` は累積 ack の単調更新と pending 削除に集中させる。

## スコープ外

- tokio task による resend loop の新設
- outbound transport への完全統合
- `AckPdu` の wire 形状変更
- system message serializer
- remote DeathWatch 統合

## 検証

- `rtk cargo test -p fraktor-remote-adaptor-std-rs`
- `rtk cargo clippy -p fraktor-remote-adaptor-std-rs -- -D warnings`
- `rtk ./scripts/ci-check.sh ai dylint`
