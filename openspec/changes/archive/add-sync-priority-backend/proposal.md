# SyncPriorityBackend を追加する提案

## 背景
同期キュー向けに優先度付きのバックエンドを用意したいが、現状は旧実装 `queue_old::priority` を利用する以外の手段がない。新しいキュー API と整合した同期優先度バックエンドを整備し、`PriorityMessage::get_priority()` が返す `Option<i8>` を解釈してメッセージごとに優先度を指定できるようにすることで、`SyncQueue<PriorityKey, _>` を用いた新実装に移行しやすくする。

## ゴール
- `modules/utils-core/src/collections/queue/backend/sync_priority_backend.rs` に優先度サポート付き同期バックエンド実装を追加する。
- 旧実装の挙動（複数レベルの優先度、最小要素の確認、溢れ制御）を新バックエンドで再現しつつ、`PriorityMessage::get_priority()` が返す `Option<i8>` を解釈してメッセージ単位で優先度を指定できるようにする。
- 新実装を検証する単体テストを `modules/utils-core/src/collections/queue/backend/tests/sync_priority_backend/tests.rs` など所定の配置に追加する。

## 非ゴール
- 非同期バックエンド (`AsyncPriorityBackend`) の実装は含めない。
- 旧 `queue_old` モジュールの削除や大規模移行は対象外。

## 成功指標
- 新バックエンドを利用したテストが全て成功し、既存テストも破綻しない。
- 優先度レベルを跨いだ `offer`/`poll`/`peek_min` の挙動が期待通りである。

## リスクと懸念
- 優先度レベルごとのストレージ成長・溢れ制御の扱いに不整合が生じる可能性。
- 旧実装の仕様が十分に文書化されていないため、解釈差異が発生する恐れ。

## 代替案
- 旧実装をラップするアダプターを作る案もあるが、廃止予定モジュールへの依存が継続するため採用しない。
