# リサーチログ: fraktor-streams

## Summary
- Pekko Streams の Source/Flow/Sink は Graph として定義され、Source は一つの出力、Flow は入出力、Sink は一つの入力という形状を持つことが明示されている。DSL は `via`/`to` の合成時に `LinearTraversalBuilder` を積み上げて Graph を構成している。
- Materializer は ActorSystem に紐づく実行基盤であり、スケジューラ/実行コンテキストを提供しつつ materialize/shutdown のライフサイクルを管理する。SystemMaterializer と Guardian により管理される点が重要。
- 実行内部は `StreamLayout`/`Modules` によるモジュール木、`GraphInterpreter`/`ActorGraphInterpreter` によるステージ駆動という分離がある。fraktor-streams も actor 上での駆動責務を明確にする必要がある。

## Research Log

### Pekko Streams の Source/Flow/Sink 形状
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Source.scala`
  - Source は「一つの出力を持つストリーム処理ステップ」であり、Graph として表現される。
  - `via`/`to` は traversal を積み上げ、Materialized 値の合成規則（Keep）を適用する。
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala`
  - Flow は「一つの入力と一つの出力を持つ」ステージである。
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Sink.scala`
  - Sink は「一つの入力を持つ」ステージであり、Subscriber として扱える概念が明示されている。
- 設計への示唆:
  - fraktor-streams の最小コアは Source/Flow/Sink の形状（入出力）と Graph 合成の境界を明確にすべき。
  - マテリアライズ値合成（Keep 相当）は Flow/Source の接続時点で契約化する必要がある。
  - DSL は `via`/`to` で Graph を合成する前提に合わせて最小コンビネータを設計する。

### Pekko Materializer の責務
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/Materializer.scala`
- 要点:
  - Materializer はストリームのブループリント（Graph）を実行中のストリームへ変換する。
  - 実行中ストリームに必要な実行コンテキストとタイマ機能を提供する。
  - shutdown により Materializer のライフサイクルを終了し、再利用不可となる。
- 設計への示唆:
  - fraktor-streams の Materializer は materialize/start/shutdown を分離し、実行資源を保持する責務を明確化する。
  - std 側は ActorSystem に統合し、Materializer がスケジューラ/実行コンテキストを提供する構造を採用する。

### Pekko ActorMaterializer / SystemMaterializer
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/ActorMaterializer.scala`
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/SystemMaterializer.scala`
- 要点:
  - ActorMaterializer は ActorRefFactory を前提にし、ActorSystem/ActorContext の寿命と連動する。
  - SystemMaterializer がシステム拡張として materializer を管理し、Guardian が lifecycle を担う。
- 設計への示唆:
  - fraktor-streams の ActorMaterializer は ActorSystem に紐づく生成経路を持ち、停止時に全ストリームを終了できるべき。
  - materializer 管理は ActorSystem に統合される前提で設計する。

### Pekko 内部実行モデル（StreamLayout / GraphInterpreter）
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/impl/StreamLayout.scala`
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/impl/Modules.scala`
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/impl/fusing/GraphInterpreter.scala`
- 出典: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/impl/fusing/ActorGraphInterpreter.scala`
- 要点:
  - モジュール木とステージ形状が明確に分離され、GraphInterpreter がステージロジックを駆動する。
  - ActorGraphInterpreter が ActorSystem と接続し、駆動ループを Actor として実行する。
- 設計への示唆:
  - fraktor-streams でも actor 上の駆動責務を `StreamDriveActor` に明示し、core と実行を分離する。

## Architecture Pattern Evaluation
- データフロー型の構成（Source/Flow/Sink → RunnableGraph → Materializer）を維持することで、Graph 合成と実行責務を分離できる。
- Materializer は ActorSystem に紐づけ、実行駆動は actor で行うことで Pekko と同様の実行モデルを確保できる。
- core は no_std を維持し、std 側で actor 実行を担う分離が最も低リスク。

## Decisions (Draft)
- `modules/streams` に独立クレート `fraktor-streams-rs` を追加し、`core`/`std` を分離する。
- Source/Flow/Sink と RunnableGraph をコアで提供し、Materializer は core と std で二層化する。
- backpressure は demand 伝播 + `modules/utils` キューにより実現する。
- demand は `Finite(u64)`（1..=u64::MAX）または `Unbounded` とし、request(0) は無効とする。
- マテリアライズ値の合成規則は `MatCombine`（KeepLeft/KeepRight/KeepBoth/KeepNone）で固定する。
- no_std は `StreamHandle::drive` による手動駆動を契約として提供する。
- std 側は ActorSystem 統合の ActorMaterializer を前提とし、drive は ActorGraphInterpreter 相当の actor で駆動する。

## Risks
- demand 伝播の粒度とバッファ設計が性能/メモリに直結する。
- no_std の実行駆動モデルが不明確なままだと API が肥大化する。

## Open Questions
- demand の上限/特別値（無制限）の表現方法
- 手動駆動（poll/tick）と std 実行の API 統一方法
- マテリアライズ値の合成規則（左優先/右優先/ペア化）

## Supporting References
- Akka Streams Operators Index
```
https://doc.akka.io/libraries/akka-core/current/stream/operators/index.html
```
