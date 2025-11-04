## Why
`actor-core` では `no_std + alloc` を前提に `spin::Mutex` をベースとした `SpinSyncMutex` を多用しているが、`std` が利用可能な構成ではスピンロックによる CPU 消費が無視できなくなるケースがある。また今後 `remote-std` やクラスタ機能が `std` 環境を前提に拡張された際、ロック競合時に OS へ制御を委ねられる同期原語が求められる。早期にバックエンドの切替境界を定義し、API 影響を最小化しながら標準ライブラリの `Mutex` を選択可能にしたい。

## What Changes
- `modules/utils-core` に `StdSyncMutex` ラッパーを追加し、`std` feature 有効時に `std::sync::Mutex` を内部で利用できるようにする。一方で `actor-core` では新たに導入する型エイリアス（例: `ActorCellMutex`）を介して `SpinSyncMutex` を参照し続け、`actor-std` / `remote-std` で同名 alias を `StdSyncMutex` へ差し替える。
- `SpinSyncMutex` の利用箇所を棚卸しし、`tokio` の非同期タスクなどでブロッキングを避けるべき箇所に対する設計指針を整理する。
- `actor-core` / `actor-std` / 将来追加予定の `remote-std` での feature 依存関係を更新し、ドキュメントとサンプルコードを整備する。
- `std` バックエンド有効時のベンチマークおよび回帰テスト方針を策定する。

## Impact
- `SpinSyncMutex` 利用箇所のロック特性が変化し、`std` 構成では OS ブロッキングを伴う挙動となる。
- `actor-core` の API には影響しないが、内部実装および `actor-std` の feature 構成を更新する必要がある。
- 同期性能の指標やベンチマークを見直し、`std` / `no_std` で異なる結果を扱う運用フローが必要になる。

## Scope
### Goals
1. `SpinSyncMutex` を `no_std + alloc` でも動作する共通抽象に保ちつつ、`std` feature で `std::sync::Mutex` バックエンドを選択できるようにする。
2. `SpinSyncMutex` の利用箇所を棚卸し、`tokio` 等の非同期実行コンテキストでのブロッキング回避策（`spawn_blocking` など）を設計する。
3. `actor-std` や将来の `remote-std` で利用する feature 配線とドキュメントを更新する。

### Non-Goals
- `SpinSyncMutex` の API を大幅に変更すること。
- `tokio` 向けに `tokio::sync::Mutex` 等の async 用同期原語を導入すること。
- クラスタ membership / gossip などリモート層の詳細仕様を決めること。

## Rollout Plan
1. 既存の `SpinSyncMutex` 実装および利用箇所を調査し、`std` 背景でブロッキングが許容されるかを分類する。
2. `utils-core` に `StdSyncMutex` を追加し、`actor-core` の型エイリアスを整備したうえで `actor-std` / `remote-std` から差し替えられる配線を実装する。
3. 主要なロック利用箇所でのベンチマーク／負荷テストを update し、`std` バックエンド時の挙動を確認する。
4. `actor-std` など依存クレートのドキュメントとサンプルを更新する。
5. 段階的に feature を広報し、問題がなければデフォルト構成で `std` バックエンドを有効化するか判断する。

## Risks & Mitigations
- **tokio 上でのブロッキングリスク**: 利用箇所の棚卸しで async 文脈を特定し、必要なら `spawn_blocking` や非同期別経路へ切り替える。
- **性能劣化**: ベンチマークで比較し、競合頻度の高い箇所は従来通り spin を選択できるようにする。
- **機能分岐の複雑化**: feature の命名と依存関係を簡潔に保ち、`no_std` 構成でのビルドを CI に追加する。

## Impacted APIs / Modules
- `modules/utils-core` 内の同期プリミティブ (`SpinSyncMutex` など)
- `modules/actor-core` および `modules/actor-std` の内部実装
- ベンチマークとドキュメントの同期関連セクション

## References
- 現行の `SpinSyncMutex` 実装コード
- tokio-rs のブロッキング設計ガイドライン
