## Why
- `specs/001-add-actor-runtime` で定義されたユーザーストーリー/受入シナリオに対して、`modules/actor-core/tests` には一部しか網羅されていない。特に ask 経路、Deadletter の詳細観測、ActorSystem（NoStdToolbox 固定）でのスループット/バックプレッシャー、監視イベントの計測などが不足している。
- ランタイムの信頼性検証を CI で保証するには、ActorSystem（NoStdToolbox）を前提とした受入テスト計画が必要だが現状ドキュメント化されていない。StdToolbox/Tokio 依存の検証は actor-std の範囲で扱う。

## What Changes
- actor-core 受入テストのカバレッジ方針を整理し、ユーザーストーリーごとのシナリオ→テストケース対応を明文化する。
- `modules/actor-core/tests` に追加すべき統合テストの仕様を固め、ActorSystem（`ActorSystemGeneric<NoStdToolbox>` のエイリアス）上で実行できる範囲に検証観点を絞る（std/ tokio 依存は actor-std 側で扱う）。
- Mailbox/Dispatcher/Deadletter/EventStream/Supervisor/Ask 経路に対する診断ポイントを列挙し、将来のテスト実装が満たすべき前提条件を定義する。

## Impact
- 影響する仕様: `specs/001-add-actor-runtime`
- 影響するコード/ドキュメント: `modules/actor-core/tests`, `modules/actor-core/examples/ping_pong_no_std`, `docs/guides/actor-system.md`
