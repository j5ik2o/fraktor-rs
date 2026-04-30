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

## 今回の暫定対応

`showcases/std` 配下のサンプルでは `collect_values()` を使わない方針に寄せた。

stream サンプルは `ActorSystem` backing の `ActorMaterializer` を起動し、`Source` を `Sink::collect()` などへ接続した `RunnableGraph` を `run(&mut materializer)` で materialize する形へ修正した。

## 後続タスク

`collect_values()` は公開 DSL から外す方向で整理する。

候補は次のいずれか。

- `pub(crate)` にして stream-core 内部テスト専用に閉じる
- `core::testing` 配下の明示的なテスト補助 API へ移す
- 公開のまま残す場合でも `#[deprecated]` を付け、`run_with(Sink::collect(), materializer)` への移行を促す

ただし現状では stream-core の unit/integration tests と公開 API 契約テストで大量に使われているため、単純な `pub(crate)` 化は影響範囲が大きい。

## 推奨方針

まずテスト側に `run_collect_with_materializer` 相当の補助関数を導入し、テストを `Materializer` 経由へ寄せる。その後、公開 API としての `collect_values()` を削除または非推奨化する。

最終状態では、ユーザーが stream を実行する入口は `RunnableGraph::run(&mut Materializer)` または `Source::run_with(..., &mut Materializer)` 系に集約する。
