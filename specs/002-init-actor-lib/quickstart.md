# Quickstart: Cellactor Actor Core 初期実装

## 1. 事前準備
- Rust nightly toolchain (`rustup override set nightly` で設定)。
- `modules/utils-core` のテストがグリーンであること (`./scripts/ci-check.sh all`)。
- 組込みターゲット用に `alloc` が利用可能なヒープ管理を準備。

## 2. ActorSystem を立ち上げる
```rust
use cellactor_actor_core::{ActorSystem, Behavior, BehaviorProfile, MessageQueuePolicy};
use cellactor_actor_core::supervision::{SupervisionStrategy, SupervisionDecision};

fn main() {
    let system = ActorSystem::new("quickstart-system");

    system.with_scope(|scope| {
        let profile = BehaviorProfile::new()
            .with_init(|ctx| {
                ctx.set_state(0u32);
                Ok(())
            })
            .with_next(|ctx, msg: u32| {
                let counter = ctx.state_mut();
                *counter += msg;
                Ok(())
            })
            .with_mailbox(MessageQueuePolicy::bounded(10).with_overflow_block())
            .with_supervision(SupervisionStrategy::restart(|error, stats| {
                if stats.failures() < error.retry_limit() {
                    SupervisionDecision::Restart
                } else {
                    SupervisionDecision::Stop
                }
            }));

        let actor = scope.spawn("counter", profile).expect("spawn actor");
        actor.tell(1);
        actor.tell(2);
        scope.observation().flush();
    });
}
```

## 3. メールボックスと Dispatcher を調整する
1. `MessageQueuePolicy::priority()` で System メッセージ優先度を上げる。
2. `DispatcherSelector::round_robin().with_throughput(50)` で公平性を高める。
3. 構成を OpenAPI (`contracts/control-plane.yaml`) の `POST /actor-system/scopes/{scopeId}/mailboxes` と照合し、設定値が仕様どおりであることを確認。

## 4. Supervision を検証する
1. 統合テストで `scope.run_probe()` を呼び出し、`SupervisionProbeResult.decision` が期待通りになるかを確認。
2. 致命的エラー (`ActorError::fatal`) を返すハンドラを仕込み、`Stop` が発火することをメトリクスで確認。
3. `contracts/control-plane.yaml` の `POST /actor-system/scopes/{scopeId}/supervision/probes` シナリオを用いて、再起動回数が `restartLimit` を超えたときに停止することを検証。

## 5. EventStream を購読する
```rust
system.with_scope(|scope| {
    let (subscription, metrics_rx) = scope.event_stream().subscribe::<SystemEvent>();
    scope.event_stream().publish(SystemEvent::Started);

    // Backpressure がかかった場合のヒントを受け取る
    if let Some(metric) = metrics_rx.try_next() {
        log::info!("metric={:?}", metric);
    }

    scope.event_stream().unsubscribe(subscription);
});
```
- `publish`/`drop`/`subscribe`/`unsubscribe` が `ObservationChannel` を通じてレポートされることを `SC-005` の指標で確認。
- バックプレッシャー発生時は `contracts/control-plane.yaml` の `/event-stream/publish` 応答を再現する。

## 6. CI とドキュメント
- 実装前後で必ず `./scripts/ci-check.sh all` と `makers ci-check -- dylint` を実行。
- protoactor-go / Apache Pekko との比較結果は `research.md` へ追記し、差分理由を明文化する。
- Quickstart のコード例は no_std 対応部分と std 拡張部分を切り分け、`ActorSystem::with_scope` の利用方法を最新に保つ。
