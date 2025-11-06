# 目的
`modules/actor-core/src/typed` で整備した typed 挙動を std ランタイムでも利用できるようにし、`modules/actor-std/src` に同等の typed パッケージ構造（Actor/Props/ActorSystem/Behaviors など）を追加する。これにより std 利用者が downcast なしに typed DSL を使えるようにする。

# 背景
現状 typed API は `actor-core` のみが提供しており、`actor-std`（Tokio 等の std 実行環境用 crate）では未提供。std 側では独自の `Actor` トレイトと adapter（`actor_prim/actor_adapter.rs`）が存在するため、その構造に沿った typed トレイト + アダプターを追加する必要がある。

# スコープ
- `modules/actor-std/src/typed/...` ディレクトリとモジュールを追加し、core 版 typed と同等の API を std 向けに提供
- std 用 typed トレイト (`typed::actor_prim::TypedActor` 等) と adapter を実装して、std `Actor` と互換を取る
- `TypedActorSystem` を `CoreTypedActorSystemGeneric<StdToolbox>` のラッパーとして実装し、std 仕様に沿った API (`when_terminated`, `terminate` など) を提供
- 代表的な example（Behaviors::setup/receiveMessage/receiveSignal 等）を std crate でも動作検証できるよう更新 or 追加

# 非スコープ
- core 側 typed 実装の API 変更
- 非 std ランタイム（no_std）向け拡張

# 成功基準
- `actor-std` crate の利用者が `typed::Behaviors` や `TypedActorSystem` を std コンテキストで利用できる
- std typed トレイト＋アダプター／Props／ActorSystem／Behaviors がコンパイル通過し、既存 CI (`./scripts/ci-check.sh all`) が成功する
