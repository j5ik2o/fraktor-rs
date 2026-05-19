# remote actor ref 解決キャッシュ実装計画

## 目的

Phase 2 medium の対象として、`ActorRefResolveCache` と `RemoteActorRef` 解決時の cache hit/miss 観測を実装する。

前バッチで `MiscMessageSerializer` の provider 非依存 subset は完了済みである。`ActorIdentity` serialization は provider resolve が前提になるため、先に resolve cache と観測経路を閉じる。

## 対象

| 領域 | 内容 |
|------|------|
| actor-core | `modules/actor-core/src/core/kernel/serialization/` に bounded resolve cache と outcome 型を追加する |
| remote-core | hit/miss event payload と `EventPublisher` の extension publish を追加する |
| remote-adaptor-std | `StdRemoteActorRefProvider` の remote resolve branch に cache と hit/miss publish を配線する |

## 実装順序

1. `ActorRefResolveCache` / outcome 型を actor-core に追加する。
2. remote-core に cache hit/miss event と `EventPublisher::publish_extension` を追加する。
3. `StdRemoteActorRefProvider` に cache と publisher を注入し、remote branch の resolve を cache 経由にする。

## スコープ外

- `ActorIdentity` serialization
- `RemoteRouterConfig` serialization
- concrete remote `ActorRef` construction
- remote send path
- Pekko ThreadLocal extension の移植

## 検証

- 変更範囲に対応する clippy / test を実行する。
- `./scripts/ci-check.sh ai dylint` を実行する。
- Fake Gap チェックとして wrapper/alias 偽装、fallback/no-op API、public/internal 境界悪化を確認する。
