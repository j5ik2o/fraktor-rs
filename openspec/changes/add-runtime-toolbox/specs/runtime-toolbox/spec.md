## ADDED Requirements
### Requirement: ランタイムは RuntimeToolbox を型引数で選択できる API を提供する
ランタイムは `ActorSystemGeneric<TB>` のように `RuntimeToolbox` を型引数として選択する手段を提供しなければならない (MUST)。既存の `ActorSystem` は `NoStdToolbox` を既定値とする型エイリアスで互換性を維持しなければならない (MUST)。

#### Scenario: 環境未指定で初期化した
- **GIVEN** 利用者が既存の `ActorSystem::new` を用いて初期化する
- **WHEN** ランタイムが同期プリミティブを生成する
- **THEN** `NoStdToolbox` の `SpinSyncMutex` バックエンドが使用され、従来通りの挙動を示す

#### Scenario: StdToolbox を指定した
- **GIVEN** 利用者が `ActorSystemGeneric<StdToolbox>::new` もしくは `StdActorSystem::new` を呼び出す
- **WHEN** ランタイムが同期プリミティブを生成する
- **THEN** `std::sync::Mutex` バックエンドが利用される

#### Scenario: 初期化後に環境を変更しようとした
- **GIVEN** `ActorSystem` が `RuntimeToolbox` を設定して初期化済みである
- **WHEN** 利用者が初期化後に別の環境へ変更しようとする
- **THEN** API により拒否されるか、明示的なエラーが返され、環境は不変である

### Requirement: RuntimeToolbox は SyncMutexLike を関連型として提供しなければならない
`RuntimeToolbox` は `SyncMutexLike` を実装する関連型 `SyncMutex<T>` を公開しなければならない (MUST)。標準環境として `NoStdToolbox` と `StdToolbox` を提供しなければならない (MUST)。

#### Scenario: NoStdToolbox を利用した
- **GIVEN** `NoStdToolbox` を使用して `SyncMutex` を生成する
- **WHEN** ランタイムがロックを生成する
- **THEN** `SpinSyncMutex` が返り、`no_std` 構成でコンパイルできる

#### Scenario: StdToolbox を利用した
- **GIVEN** `StdToolbox` を使用して `SyncMutex` を生成する
- **WHEN** ランタイムがロックを生成する
- **THEN** `std` feature が有効な環境で `StdSyncMutex` が生成される

#### Scenario: カスタム環境を実装した
- **GIVEN** 利用者が独自の `RuntimeToolbox` を実装し、`ActorSystemGeneric<MyToolbox>` を構築する
- **WHEN** ランタイムが同期プリミティブを生成する
- **THEN** カスタム環境が提供する `SyncMutexLike` 実装が利用される

### Requirement: ドキュメントで環境設定手順を示さなければならない
ランタイムは `RuntimeToolbox` の設定方法と注意点をドキュメント化しなければならない (MUST)。

#### Scenario: actor-std 利用者がガイドを参照した
- **GIVEN** `actor-std` を利用する開発者がドキュメントを確認する
- **WHEN** `StdToolbox` を設定する手順を読む
- **THEN** 必要な feature とコード例が記載されている
