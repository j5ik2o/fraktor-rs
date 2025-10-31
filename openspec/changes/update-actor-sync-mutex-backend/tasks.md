## 1. 調査と設計整理
- [ ] `SpinSyncMutex` の現行実装と利用箇所を洗い出し、`std` 環境でブロッキングが許容されるかを分類する
- [ ] `tokio` など非同期実行コンテキストでの利用箇所を特定し、ブロッキング回避方針（`spawn_blocking` 等）を検討する
- [ ] CI / ベンチマークの対象に `std` 構成を追加する際の影響をまとめる

## 2. 実装
- [ ] `modules/utils-core` に `StdSyncMutex` を追加し、`std` feature で `std::sync::Mutex` をラップする実装を提供する
- [ ] `actor-core` に `ActorCellMutex` 等の型エイリアスを導入し、デフォルトで `SpinSyncMutex` を参照させる
- [ ] `actor-std` / 将来の `remote-std` から同名 alias を `StdSyncMutex` に差し替える再エクスポートを実装する
- [ ] `no_std + alloc` 構成との互換性を保つためのコンパイル時テストを整備する
- [ ] Cargo feature の依存関係とドキュメントを更新する

## 3. 検証とドキュメント
- [ ] `std` バックエンド有効時のベンチマークや負荷テストを実施し、従来構成と差分を記録する
- [ ] ブロッキングが問題となる箇所がないかコードレビューを行い、必要に応じて `spawn_blocking` 等で回避する
- [ ] ドキュメントとサンプルコードを更新し、利用者向けの切替手順を追記する
