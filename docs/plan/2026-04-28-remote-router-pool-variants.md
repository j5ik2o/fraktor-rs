# remote router pool variants 実装計画

## 背景
TAKT `pekko-porting` ワークフローの Phase 2 として、`RemoteRouterConfig` が扱える local pool variant を拡張する。
Phase 1 の outbound `maximum_frame_size` enforcement は前バッチで完了済みのため、今回の実装対象は `RoundRobinPool` / `RandomPool` と serializer 対応に限定する。

## 実装対象
- `RoundRobinPool` を routing kernel に追加する。
- `RandomPool` を routing kernel に追加する。
- `routing.rs` の module wiring と public export を更新する。
- `MiscMessageSerializer` で `RemoteRouterConfig<RoundRobinPool>` / `RemoteRouterConfig<RandomPool>` を encode / decode できるようにする。
- built-in serialization registry に上記 2 型の binding を追加する。

## 実装しないもの
- remote routee expansion は Phase 3 の concrete remote `ActorRef` construction に依存するため扱わない。
- `BroadcastPool` は現行 routing core が単一 routee 選択中心であり、全 routee 配送契約を no-op なく実装できないため扱わない。
- `BalancingPool` は dispatcher / mailbox 共有契約が未整備のため扱わない。
- advanced Artery settings は runtime wiring なしの設定追加が no-op API になるため扱わない。

## 実装順序
1. routing kernel に `RoundRobinPool` / `RandomPool` を追加する。
2. `routing.rs` の module wiring と public export を更新する。
3. `MiscMessageSerializer` の pool tag と encode / decode 分岐を追加する。
4. built-in serializer default binding を追加する。
5. actor-core の lint / テスト / dylint を実行する。
