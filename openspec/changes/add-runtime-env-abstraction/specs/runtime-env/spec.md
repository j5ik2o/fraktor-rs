## ADDED Requirements
### Requirement: ランタイムは RuntimeEnv を設定できる API を提供する
ランタイムは `ActorSystem` 初期化時に `RuntimeEnv` を設定する手段を提供しなければならない (MUST)。未設定の場合は `NoStdEnv` を使用しなければならない (MUST)。

#### Scenario: 環境未指定で初期化した
- **GIVEN** 利用者が環境を設定せずに `ActorSystem` を初期化する
- **WHEN** ランタイムが同期プリミティブを生成する
- **THEN** `NoStdEnv` の `SpinSyncMutex` バックエンドが使用され、従来通りの挙動を示す

#### Scenario: StdEnv を指定した
- **GIVEN** 利用者が `StdEnv` を設定して `ActorSystem` を初期化する
- **WHEN** ランタイムが同期プリミティブを生成する
- **THEN** `std::sync::Mutex` バックエンドが利用される

### Requirement: RuntimeEnv は SyncMutexLike を生成しなければならない
`RuntimeEnv` は `SyncMutexLike` を実装する同期プリミティブを生成しなければならない (MUST)。標準環境として `NoStdEnv` と `StdEnv` を提供しなければならない (MUST)。

#### Scenario: NoStdEnv を利用した
- **GIVEN** `NoStdEnv` を使用して `SyncMutex` を生成する
- **WHEN** ランタイムがロックを生成する
- **THEN** `SpinSyncMutex` が返り、`no_std` 構成でコンパイルできる

#### Scenario: StdEnv を利用した
- **GIVEN** `StdEnv` を使用して `SyncMutex` を生成する
- **WHEN** ランタイムがロックを生成する
- **THEN** `std` feature が有効な環境で `StdSyncMutex` が生成される

### Requirement: ドキュメントで環境設定手順を示さなければならない
ランタイムは `RuntimeEnv` の設定方法と注意点をドキュメント化しなければならない (MUST)。

#### Scenario: actor-std 利用者がガイドを参照した
- **GIVEN** `actor-std` を利用する開発者がドキュメントを確認する
- **WHEN** `StdEnv` を設定する手順を読む
- **THEN** 必要な feature とコード例が記載されている
