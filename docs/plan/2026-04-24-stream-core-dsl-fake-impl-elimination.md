# stream core DSL fake-impl 排除計画

## 目的

StreamRef local handoff の実装後に残っている stream core DSL の fake-impl / placeholder を、Pekko の契約意図に合わせて実データパスへ置き換える。

## 対象

| タスク | 対象ファイル | 方針 |
|--------|--------------|------|
| `Flow::contramap` / `Flow::dimap` | `modules/stream-core/src/core/dsl/flow.rs` | 入力側 map を元 Flow の前段に合成し、元 Flow の materialized value を保持する |
| `Flow::do_on_cancel` | `modules/stream-core/src/core/dsl/flow.rs`, `modules/stream-core/src/core/impl/fusing/do_on_cancel_logic.rs` | downstream cancel 時に callback を一度実行する FlowLogic を追加する |
| `Sink::from_materializer` | `modules/stream-core/src/core/dsl/sink.rs`, `modules/stream-core/src/core/dsl/sink/materialized_sink_logic.rs` | factory を stream 開始時に遅延実行し、生成された Sink の処理結果を materialized completion に反映する |
| `Sink::source` | `modules/stream-core/src/core/dsl/sink.rs`, `modules/stream-core/src/core/dsl/sink/sink_source_logic.rs` | Sink 入力を queue backed Source へ流す live bridge を作る |
| 固定スコープ外 alias 削除 | `flow.rs`, `sink.rs`, `source/tests.rs` | JVM `CompletionStage` / `Publisher` / `Subscriber` 互換 API を削除する |
| StreamRef 空 module cleanup | `modules/stream-core/src/core/impl/streamref.rs`, `modules/stream-core/src/core/impl/streamref/stream_ref_runtime.rs` | doc comment だけの runtime module を削除する |
| gap-analysis 更新 | `docs/gap-analysis/stream-gap-analysis.md` | StreamRef local handoff と core DSL fake-impl 排除後の状態に更新する |

## 検証

1. `cargo clippy -p fraktor-stream-core-rs -- -D warnings`
2. `cargo test -p fraktor-stream-core-rs`
3. `./scripts/ci-check.sh ai dylint`

TAKT 実行中のため `./scripts/ci-check.sh ai all` は実行しない。
