## ADDED Requirements
### Requirement: SpinSyncMutex は環境に応じたバックエンドを提供する
ランタイムは `SpinSyncMutex` において `no_std + alloc` 環境ではスピンロック実装を、`std` feature が有効な環境では `StdSyncMutex`（`std::sync::Mutex` をラップした実装）を同じ API で提供しなければならない (MUST)。`actor-core` では型エイリアスを介して `SpinSyncMutex` を参照し、`actor-std` など `std` 対応クレートは同名 alias を `StdSyncMutex` に差し替えなければならない (MUST)。

#### Scenario: std feature を有効化した
- **GIVEN** ビルド時に `std` feature が有効になり、`actor-std` が `ActorCellMutex` などの alias を `StdSyncMutex` に差し替えている
- **WHEN** ランタイムが `SpinSyncMutex` 系の型を初期化する
- **THEN** 内部で `StdSyncMutex` が選択され、`std::sync::Mutex` によるブロッキング制御が利用される

#### Scenario: no_std 構成でビルドした
- **GIVEN** `no_std + alloc` 構成で `std` feature が無効になっている
- **WHEN** `SpinSyncMutex` を初期化する
- **THEN** 従来通り `spin::Mutex` バックエンドが選択され、コンパイルが成功する

### Requirement: 非同期コンテキストでのブロッキングを検証する
ランタイムは `SpinSyncMutex` を利用する箇所が非同期タスク内でブロッキングを引き起こさないように検証しなければならない (MUST)。

#### Scenario: tokio タスク内でロックを取得した
- **GIVEN** `tokio` タスク内で `SpinSyncMutex` のロックを取得する箇所が存在する
- **WHEN** `std` feature が有効で `std::sync::Mutex` バックエンドが使用される
- **THEN** コードレビューまたはテストでブロッキング影響が評価され、必要に応じて `spawn_blocking` などの回避策が適用される

### Requirement: 利用者向けドキュメントを更新する
ランタイムは `SpinSyncMutex` のバックエンド切替手順と注意事項をドキュメント化しなければならない (MUST)。

#### Scenario: std バックエンドを利用したい開発者
- **GIVEN** 開発者が `std` 環境で `SpinSyncMutex` の `std::sync::Mutex` バックエンドを利用したい
- **WHEN** ドキュメントを参照する
- **THEN** feature 設定とブロッキングに関する注意点が記載されており、設定手順に従って切り替えられる
