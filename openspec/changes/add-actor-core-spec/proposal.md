## Why
- actor-core の基盤仕様群を分散したままにせず、統合要求を 1 つの参照仕様としてまとめる。
- Protoactor-go および Apache Pekko Typed を参照しつつ、cellactor-rs における no_std 対応やメールボックス/ディスパッチャ周辺の統一指針を示す必要がある。
- 複数の既存ドラフト (001-*) を横断して整合性を確保し、今後の実装タスクを整理する。

## What Changes
- 新規 capability `actor-core` を追加し、ActorSystem/Behavior/Props/メールボックス/Dispatcher/Supervision/EventStream/Typed 橋渡しの要件を統合する。
- 各ユーザーストーリーと対応する機能要件 (FR-*) を Requirements として明文化する。
- Result ベースエラー通知や panic 取り扱いなど、破壊的変更を許容する設計方針を記載する。

## Impact
- actor-core 実装の計画を立てる際の単一情報源となる。
- 後続の change proposal が capability 分割する際の基準として利用できる。
- テスト計画 (メールボックス契約・監視戦略) の前提が明確化されることで、CI 設計が容易になる。

## Open Questions
- Dispatcher/Invoker の no_std 実装詳細 (具体的なタイマー抽象) をどこまで仕様化するか。
- メトリクス出力インターフェイスの詳細粒度 (ラベル構成) を追加するかは別仕様に切り出すか。
