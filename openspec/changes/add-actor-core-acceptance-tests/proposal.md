## Why
- `specs/001-add-actor-runtime` で定義されたユーザーストーリー/受入シナリオに対して、`modules/actor-core/tests` には一部しか網羅されていない。特に ask 経路、Deadletter の詳細観測、ActorSystem（NoStdToolbox 固定）でのスループット/バックプレッシャー、監視イベントの計測などが不足している。
- ランタイムの信頼性検証を CI で保証するには、TokioExecutor（`DispatchExecutor` を差し替え）で ActorSystem を駆動する受入テスト計画が必要だが現状ドキュメント化されていない。Tokio ランタイムのスレッドプール設定や `spawn_blocking` 経路を使って dispatcher を走らせる観点が不足している。

## What Changes
- actor-core 受入テストのカバレッジ方針を整理し、ユーザーストーリーごとのシナリオ→テストケース対応を明文化する。
- `modules/actor-core/tests` と `modules/actor-std/examples` に追加すべき統合テストの仕様を固め、TokioExecutor を使って dispatcher を駆動するケースを中心に検証観点を整理する。
- Mailbox/Dispatcher/Deadletter/EventStream/Supervisor/Ask 経路に対する診断ポイントを列挙し、将来のテスト実装が満たすべき前提条件を定義する。

## Impact
- 影響する仕様: `specs/001-add-actor-runtime`
- 影響するコード/ドキュメント: `modules/actor-core/tests`, `modules/actor-std/src/dispatcher/dispatch_executor/tokio_executor.rs`, `modules/actor-std/examples/ping_pong_tokio`, `docs/guides/actor-system.md`
