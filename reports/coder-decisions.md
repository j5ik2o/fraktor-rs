# 実装判断ログ

## T1: ActorRef::no_sender()

- **判断**: `ActorRef::null()` へのエイリアスとして実装。Pekko の `Actor.noSender` に対応。
- **根拠**: 既存の `null()` が同じセマンティクスを持つため、委譲が最適。

## T2: DeadLetterListener

- **判断**: `std/event/stream/` に配置。`EventStreamSubscriber` trait として実装（独立アクターではない）。
- **根拠**: fraktor-rs のイベントストリームアーキテクチャに合わせた設計。Pekko ではアクターだが、fraktor-rs では subscriber trait が適切。
- **技術的制約**: `tracing::event!` マクロの target パラメータはコンパイル時定数が必要。フィールドではなくモジュールレベル `const` で対応。

## T3: ActorContext::forward() / TypedActorContext::forward()

- **判断**: untyped 層で sender 保持ロジックを実装し、typed 層は委譲のみ。
- **根拠**: ロジックを untyped kernel に集約する既存方針に従う。

## T4: Behaviors::receive_message_partial / receive_partial

- **判断**: `Option<Behavior<M>>` を返すハンドラ。`None` → `Behavior::unhandled()` に変換。
- **根拠**: Pekko の `Behaviors.receiveMessagePartial` / `Behaviors.receivePartial` の直接対応。

## T5: Props タグサポート

- **判断**: `BTreeSet<String>` で tags フィールドを追加。`with_tags()` / `with_tag()` ビルダーメソッド。
- **根拠**: `BTreeSet` は no_std 互換（`alloc`）かつ順序保証あり。Props の不変性パターンに従い新インスタンスを返す。

## T6: Address 型

- **判断**: `ActorPathScheme` / `ActorPathParts` / `PathAuthority` を再利用する設計。
- **根拠**: 既存の内部型を活用し、重複を避ける。URI フォーマットは `ActorPathScheme::as_str()` に従い `fraktor.tcp` (ドット区切り)。

## T7: TypedActorContext::schedule_once()

- **判断**: `SchedulerShared` の `with_write` API 経由で `TypedScheduler::schedule_once()` を呼び出す。
- **根拠**: 既存の scheduler 共有パターン（`SharedAccess` trait）に従い、ロックスコープをメソッド内に閉じる。

## CI 結果

- **clippy**: `cargo clippy -p fraktor-actor-rs --features std -- -D warnings` → 警告なし ✅
- **テスト**: `cargo test -p fraktor-actor-rs --features std` → 804 passed, 0 failed ✅
- **全体CI**: `./scripts/ci-check.sh ai all` → 900秒タイムアウト（全テストパス確認済み、ハング検出は既存の統合テストの遅延が原因）
