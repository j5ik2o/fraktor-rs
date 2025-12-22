# リサーチログ: fraktor-streams

## Summary
- Pekko Streams の Source/Flow/Sink は Graph として定義され、Source は一つの出力、Flow は入出力、Sink は一つの入力という形状を持つことが明示されている。これは fraktor-streams の最小 API で採用する構成単位の根拠になる。
- Materializer はストリームブループリントを実行中ストリームに変換し、実行資源（スケジューラ/実行コンテキスト）を提供し、shutdown によりライフサイクルを終了する責務を持つ。これを fraktor-streams の Materializer 契約の基礎にする。
- ストリーム処理は actor の責務と分離し、`modules/streams` に独立配置する方針が core/std 境界の維持に適合する。

## Research Log

### Pekko Streams の Source/Flow/Sink 形状
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Source.scala`
  - Source は「一つの出力を持つストリーム処理ステップ」であり、Graph として表現される。
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala`
  - Flow は「一つの入力と一つの出力を持つ」ステージである。
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Sink.scala`
  - Sink は「一つの入力を持つ」ステージであり、Subscriber として扱える概念が明示されている。
- 設計への示唆:
  - fraktor-streams の最小コアは Source/Flow/Sink の形状（入出力）と Graph 合成の境界を明確にすべき。
  - マテリアライズ値合成（Keep 相当）は Flow/Source の接続時点で契約化する必要がある。

### Pekko Materializer の責務
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/Materializer.scala`
- 要点:
  - Materializer はストリームのブループリント（Graph）を実行中のストリームへ変換する。
  - 実行中ストリームに必要な実行コンテキストとタイマ機能を提供する。
  - shutdown により Materializer のライフサイクルを終了し、再利用不可となる。
- 設計への示唆:
  - fraktor-streams の Materializer は materialize/start/shutdown を分離し、実行資源を保持する責務を明確化する。
  - no_std では実行資源を外部注入とし、std 側で tokio ブリッジを提供する。

## Architecture Pattern Evaluation
- データフロー型の構成（Source/Flow/Sink → RunnableGraph → Materializer）を維持することで、Graph 合成と実行責務を分離できる。
- actor モジュールに統合せず、独立した streams クレートとして境界を固定するのが最も低リスク。

## Decisions (Draft)
- `modules/streams` に独立クレート `fraktor-streams-rs` を追加し、`core`/`std` を分離する。
- Source/Flow/Sink と RunnableGraph をコアで提供し、Materializer は core と std で二層化する。
- backpressure は demand 伝播 + `modules/utils` キューにより実現する。
- demand は `Finite(u64)`（1..=u64::MAX）または `Unbounded` とし、request(0) は無効とする。
- マテリアライズ値の合成規則は `MatCombine`（KeepLeft/KeepRight/KeepBoth/KeepNone）で固定する。
- no_std は `StreamHandle::drive` による手動駆動を契約として提供する。

## Risks
- demand 伝播の粒度とバッファ設計が性能/メモリに直結する。
- no_std の実行駆動モデルが不明確なままだと API が肥大化する。

## Open Questions
- demand の上限/特別値（無制限）の表現方法
- 手動駆動（poll/tick）と std 実行の API 統一方法
- マテリアライズ値の合成規則（左優先/右優先/ペア化）
