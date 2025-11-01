## ADDED Requirements
### Requirement: ランタイムは RuntimeToolbox を設定できる API を提供する
ランタイムは `ActorSystem` 初期化時に `RuntimeToolbox` を設定する手段を提供しなければならない (MUST)。未設定の場合は `NoStdToolbox` を使用しなければならない (MUST)。

#### Scenario: 環境未指定で初期化した
- **GIVEN** 利用者が環境を設定せずに `ActorSystem` を初期化する
- **WHEN** ランタイムが同期プリミティブを生成する
- **THEN** `NoStdToolbox` の `SpinSyncMutex` バックエンドが使用され、従来通りの挙動を示す

#### Scenario: StdToolbox を指定した
- **GIVEN** 利用者が `StdToolbox` を設定して `ActorSystem` を初期化する
- **WHEN** ランタイムが同期プリミティブを生成する
- **THEN** `std::sync::Mutex` バックエンドが利用される

#### Scenario: 初期化後に環境を変更しようとした
- **GIVEN** `ActorSystem` が `RuntimeToolbox` を設定して初期化済みである
- **WHEN** 利用者が初期化後に別の環境へ変更しようとする
- **THEN** API により拒否されるか、明示的なエラーが返され、環境は不変である

### Requirement: RuntimeToolbox は SyncMutexLike を生成しなければならない
`RuntimeToolbox` は `SyncMutexLike` を実装する同期プリミティブを生成しなければならない (MUST)。標準環境として `NoStdToolbox` と `StdToolbox` を提供しなければならない (MUST)。

#### Scenario: NoStdToolbox を利用した
- **GIVEN** `NoStdToolbox` を使用して `SyncMutex` を生成する
- **WHEN** ランタイムがロックを生成する
- **THEN** `SpinSyncMutex` が返り、`no_std` 構成でコンパイルできる

#### Scenario: StdToolbox を利用した
- **GIVEN** `StdToolbox` を使用して `SyncMutex` を生成する
- **WHEN** ランタイムがロックを生成する
- **THEN** `std` feature が有効な環境で `StdSyncMutex` が生成される

#### Scenario: カスタム環境を実装した
- **GIVEN** 利用者が独自の `RuntimeToolbox` を実装し、`ActorSystemBuilder` に設定する
- **WHEN** ランタイムが同期プリミティブを生成する
- **THEN** カスタム環境が返す `SyncMutexLike` 実装が利用される

### Requirement: ドキュメントで環境設定手順を示さなければならない
ランタイムは `RuntimeToolbox` の設定方法と注意点をドキュメント化しなければならない (MUST)。

#### Scenario: actor-std 利用者がガイドを参照した
- **GIVEN** `actor-std` を利用する開発者がドキュメントを確認する
- **WHEN** `StdToolbox` を設定する手順を読む
- **THEN** 必要な feature とコード例が記載されている
