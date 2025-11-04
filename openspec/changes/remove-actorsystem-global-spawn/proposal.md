## Why
- 現在の `ActorSystem` は `spawn` や `spawn_child`、`actor_ref` などを外部 API として公開しており、アクター階層やスーパービジョン境界を簡単に飛び越えられてしまう。
- Pekko/Akka Typed ではアクター生成や停止は `ActorContext`（=アクター内部）に限定されており、ランタイム一貫性・セーフティを担保している。
- cellactor-rs でも同等の境界を導入し、`ActorSystem` はガーディアン起動とメトリクス/イベントストリームへのアクセスだけを受け持つべき。

## What Changes
- `ActorSystemGeneric` から `spawn` / `spawn_child` / `actor_ref` / `children` / `stop_actor` などアクター内部専用 API を削除し、`ActorContext`（または専用ユーティリティ）に移行する。
- `ActorContext` API を拡充して、既存の生成・停止呼び出しをすべてこちらにリダイレクトする。
- 破壊的変更になるため、関連サンプル・テスト・ドキュメントをアップデートする。

## Impact
- 既存コードで `ActorSystem::spawn` 等を直接呼んでいる箇所はコンパイルエラーになる。アクター内部の `ctx.spawn` に書き換える必要がある。
- ランタイム内部（ガーディアン初期化やテストユーティリティ）では専用ヘルパを用意し、外部 API からは見えないようにする。
- ActorRef 探索・停止もアクター内部責務になるため、デバッグ系のテストはガーディアン経由のメッセージ駆動に切り替える。
