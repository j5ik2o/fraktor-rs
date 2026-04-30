# Stream `collect_values()` 公開 API 整理メモ

## 背景

`Source::collect_values()` は `ActorSystem` や `Materializer` を受け取らず、内部で `Stream::new(...).start()` と `drive()` を直接呼び出してストリームを同期実行している。

これは fraktor-rs のテストや簡易評価では便利だが、Apache Pekko Streams の利用モデルとは異なる。Pekko では stream は graph blueprint を作るだけでは実行されず、`ActorSystem` に紐づく `Materializer` で materialize して初めて動作する。

## 問題

`collect_values()` が公開 API のままだと、ユーザー向けサンプルで `ActorSystem` なしに stream が実行できるように見える。

これは次の誤解を招く。

- fraktor-rs の stream が actor runtime から独立して動作するように見える
- Pekko 互換の materialization 契約が不要に見える
- `RunnableGraph::run(&mut Materializer)` と `ActorMaterializer` の位置づけが曖昧になる

## 対応

`showcases/std` 配下のサンプルでは `collect_values()` を使わない方針に寄せた。

stream サンプルは `ActorSystem` backing の `ActorMaterializer` を起動し、`Source` を `Sink::collect()` などへ接続した `RunnableGraph` を `run(&mut materializer)` で materialize する形へ修正した。

さらに stream-core の公開 DSL から `collect_values()`、`TailSource::collect_values()`、`into_input_stream()`、`into_java_stream()`、`into_output_stream()` を削除した。これにより、利用者が `ActorSystem` / `Materializer` なしで stream を実行できるように見える公開入口はなくなった。

stream-core の unit/integration tests は、テスト専用 helper を通じて `run_with(Sink::collect(), &mut materializer)` または `ActorMaterializer` 経由で materialize する形へ移行した。helper は公開 API ではなく、実装も Source を直接同期実行せず materializer 契約を通す。

## 残課題

`Source::lazy_source` の内部では nested source を評価するための private な drain 処理が残っている。これは公開 API ではないが、Pekko 互換の materializer-context 実行へ寄せる余地がある。

最終状態では、ユーザーが stream を実行する入口は `RunnableGraph::run(&mut Materializer)` または `Source::run_with(..., &mut Materializer)` 系に集約する。
