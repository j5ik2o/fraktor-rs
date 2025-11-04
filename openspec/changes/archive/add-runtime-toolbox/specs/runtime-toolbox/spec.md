## ADDED Requirements
### Requirement: ランタイムは RuntimeToolbox を公開 API を変えずに選択できるようにしなければならない
ランタイムは `RuntimeToolbox` を差し替える手段を提供しつつ、既存の `ActorSystem` / `ActorRuntimeMutex` などの公開型シグネチャを維持しなければならない (MUST)。標準環境として `NoStdToolbox` と `StdToolbox` を提供し、利用者はエイリアスまたはビルダー API を通じて選択できなければならない (MUST)。

#### Scenario: 環境未指定で初期化した
- **GIVEN** 利用者が既存の `ActorSystem::new` を用いて初期化する
- **WHEN** ランタイムが同期プリミティブを生成する
- **THEN** `NoStdToolbox` のバックエンドが使用され、従来通りの挙動を示す

#### Scenario: StdToolbox エイリアスを利用した
- **GIVEN** 利用者が `StdActorSystem::new` もしくは `ActorSystemBuilder::with_toolbox::<StdToolbox>()` を用いる
- **WHEN** ランタイムが同期プリミティブを生成する
- **THEN** `std::sync::Mutex` バックエンドが利用される

#### Scenario: 初期化後に環境を変更しようとした
- **GIVEN** `ActorSystem` が `RuntimeToolbox` を設定して初期化済みである
- **WHEN** 利用者が初期化後に別の環境へ変更しようとする
- **THEN** API により拒否されるか、明示的なエラーが返され、環境は不変である

### Requirement: RuntimeToolbox は SyncMutexFamily を通じてミューテックス生成を提供しなければならない
`RuntimeToolbox` は `SyncMutexFamily` を関連型として提供し、`SyncMutexFamily::create` で `SyncMutexLike` を実装するミューテックスを生成できなければならない (MUST)。標準環境として `SpinMutexFamily` と `StdMutexFamily` を用意し、それぞれ `NoStdToolbox` / `StdToolbox` を通じて公開しなければならない (MUST)。

#### Scenario: NoStdToolbox を利用した
- **GIVEN** `NoStdToolbox` のファミリーで `SyncMutexFamily::create` を呼び出す
- **WHEN** ランタイムがロックを生成する
- **THEN** `SpinSyncMutex` が返り、`no_std` 構成でコンパイルできる

#### Scenario: StdToolbox を利用した
- **GIVEN** `StdToolbox` のファミリーで `SyncMutexFamily::create` を呼び出す
- **WHEN** ランタイムがロックを生成する
- **THEN** `std` feature が有効な環境で `StdSyncMutex` が生成される

#### Scenario: カスタム環境を実装した
- **GIVEN** 利用者が独自の `SyncMutexFamily` / `RuntimeToolbox` を実装し、ビルダー API 経由で注入する
- **WHEN** ランタイムが同期プリミティブを生成する
- **THEN** カスタム環境が提供する `SyncMutexLike` 実装が利用される

### Requirement: ドキュメントで環境設定手順を示さなければならない
ランタイムは `RuntimeToolbox` の設定方法と注意点をドキュメント化しなければならない (MUST)。

#### Scenario: actor-std 利用者がガイドを参照した
- **GIVEN** `actor-std` を利用する開発者がドキュメントを確認する
- **WHEN** `StdToolbox` を選択する手順を読む
- **THEN** 必要な feature とコード例が記載されている
