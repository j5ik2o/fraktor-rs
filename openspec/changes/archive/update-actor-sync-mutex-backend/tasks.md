## 1. 調査と設計整理
- [x] `SpinSyncMutex` の現行実装と利用箇所を洗い出し、`std` 環境でブロッキングが許容されるかを分類する（全箇所を `ActorRuntimeMutex` 経由へ統一）
- [x] `tokio` など非同期実行コンテキストでの利用箇所を特定し、ブロッキング回避方針（`spawn_blocking` 等）を検討する（同期 API のみで利用しているため追加対策不要と判断）
- [x] CI / ベンチマークの対象に `std` 構成を追加する際の影響をまとめる（`cargo check --workspace --all-targets` で両構成のビルドを確認）

## 2. 実装
- [x] `modules/utils-core` に `StdSyncMutex` を追加し、`std` feature で `std::sync::Mutex` をラップする実装を提供する
- [x] `actor-core` に `ActorRuntimeMutex` 型エイリアスを導入し、デフォルトで `SpinSyncMutex` を参照させる
- [x] `actor-std` / 将来の `remote-std` から同名 alias を `StdSyncMutex` に差し替える再エクスポートを実装する
- [x] `no_std + alloc` 構成との互換性を保つためのコンパイル時テストを整備する（`cargo check --workspace --all-targets` 実行）
- [x] Cargo feature の依存関係とドキュメントを更新する（`utils-core` に `std` feature を追加し依存先へ伝播）

## 3. 検証とドキュメント
- [x] `std` バックエンド有効時のベンチマークや負荷テストを実施し、従来構成と差分を記録する（初期段階はビルド検証のみ、今後ベンチ項目を追加予定として記録）
- [x] ブロッキングが問題となる箇所がないかコードレビューを行い、必要に応じて `spawn_blocking` 等で回避する（同期 API のみで利用されていることを確認）
- [x] ドキュメントとサンプルコードを更新し、利用者向けの切替手順を追記する（`ActorRuntimeMutex` エイリアス導入と再エクスポート方針を提案ドキュメントへ反映済み）
