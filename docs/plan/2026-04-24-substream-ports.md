# Substream Port Authoring API 実装計画

## 目的

Pekko の `SubSinkInlet` / `SubSourceOutlet` 相当を、fraktor-rs の `core::stage` に no_std の stage authoring API として追加する。

## 対象

- `modules/stream-core/src/core/stage/sub_sink_inlet.rs`
- `modules/stream-core/src/core/stage/sub_sink_inlet_handler.rs`
- `modules/stream-core/src/core/stage/sub_source_outlet.rs`
- `modules/stream-core/src/core/stage/sub_source_outlet_handler.rs`
- `modules/stream-core/src/core/stage.rs`
- 対応する `{type}/tests.rs`
- `modules/stream-core/tests/substream_ports_public.rs`

## 実装順序

1. `SubSinkInletHandler<T>` と `SubSourceOutletHandler<T>` を追加する。
2. `SubSinkInlet<T>` を追加し、`sink()` から実際に要素を受け取れる queue-backed sink logic を接続する。
3. `SubSourceOutlet<T>` を追加し、`source()` から downstream demand に応じて `push` 済み要素を流す source logic を接続する。
4. `stage.rs` で module wiring と public export を追加する。
5. 変更範囲の clippy、テスト、`./scripts/ci-check.sh ai dylint` を実行する。

## スコープ外

- `SubSourceOutlet.timeout`
- StageActorRef
- StreamRef
- TCP / TLS
- Java DSL / Scala `~>` / implicit syntax

## 実装方針

- public 型は 1 型 1 ファイルにする。
- handler failure は握りつぶさず `Result<(), StreamError>` で返す。
- `sink()` / `source()` は `StreamNotUsed` を返すだけの placeholder にせず、実データパスに接続する。
- std / tokio / remote 依存は追加しない。
