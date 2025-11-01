## Why
`ActorRuntimeMutex` は利用クレートに応じて暗黙的にバックエンドが切り替わるが、アプリケーション側からは挙動を明示できず学習コストが高い。今後 `std` / `embedded` で異なる同期プリミティブを提供したい場合、共通の注入ポイントが必要になる。`RuntimeToolbox` 抽象を導入し、ランタイムが利用する同期プリミティブを一括して管理できるようにすることで、利用者は明示的にバックエンドを選択でき、将来的な拡張にも備えられる。

## What Changes
- `SyncMutexFamily` トレイトを新設し、`type Mutex<T>` と `fn create(value: T)` を通じてミューテックス生成を統一する。
- `RuntimeToolbox` は `type MutexFamily: SyncMutexFamily` を提供するだけの薄いラッパーへ再定義し、`NoStdToolbox` / `StdToolbox` を実装する。
- ランタイム内部では `ToolboxMutex<T, TB>`（=`<TB::MutexFamily as SyncMutexFamily>::Mutex<T>`）のような型エイリアスで同期プリミティブを取得し、公開 API の `ActorRuntimeMutex<T>` は従来通り `NoStdToolbox` バックエンドを指す。
- `ActorSystem` / `ActorSystemBuilder` など公開型はジェネリクスを露出させずに維持しつつ、内部的には選択された `RuntimeToolbox` を保持できるようにする（例: ビルダーに `with_toolbox::<StdToolbox>()` / `with_toolbox_family(StdToolbox)` を追加し、別名 `StdActorSystem` を提供）。
- ドキュメントとサンプルを更新し、利用者がエイリアスやビルダー API を通じてミューテックスバックエンドを切り替える手順を示す。

## Impact
- 既存の `ActorSystem::new` や `ActorRuntimeMutex<T>` を利用するコードは変更不要のまま、`StdActorSystem` などのエイリアスやビルダー API 経由でバックエンドを明示的に選択できるようになる。
- ミューテックス生成は `SyncMutexFamily::create` に一本化されるため、動的ディスパッチなしで異なるバックエンドへ切り替えられる。
- 将来的に `Condvar` などを追加する場合は `SyncMutexFamily` と同様の *Family* トレイトを導入し、`RuntimeToolbox` に関連型を追加するだけで拡張できる。

## Scope
### Goals
1. `SyncMutexFamily` / `RuntimeToolbox` の二段構えを導入し、`StdToolbox` / `NoStdToolbox` など既定環境を実装する。
2. ランタイム内部のロック生成を `SyncMutexFamily::create` に置き換え、`ActorRuntimeMutex<T>` は既定バックエンドの型エイリアスとして維持する。
3. `ActorSystem` / `ActorSystemBuilder` の公開 API では追加の型引数を露出させずに、内部的に `RuntimeToolbox` を差し替えられる仕組み（ビルダー API や型エイリアス）を用意する。
4. `actor-std` の再エクスポートとドキュメントを更新し、利用者が `StdToolbox` 相当を選択する手順を示す。

### Non-Goals
- ランタイム起動後の `RuntimeToolbox` 動的変更。
- `Condvar` 等の新たな同期プリミティブ本体の実装（別提案で扱う）。
- 既存公開 API を総ジェネリクス化すること（表層は型エイリアスやビルダーオプションで包む）。

## Rollout Plan
1. `ActorRuntimeMutex::new` / `SpinSyncMutex::new` を直接呼ぶ箇所を洗い出し、`SyncMutexFamily::create` に差し替える対象を把握する。
2. `SyncMutexFamily` / `RuntimeToolbox` / `NoStdToolbox` / `StdToolbox` を実装し、単体テストで `create` から適切なミューテックスが得られることを確認する。
3. ランタイム内部構造体（`ActorSystemState` / `Mailbox` 等）を `ToolboxMutex<T, TB>` に移行しつつ、公開 API は既存シグネチャを維持する。
4. `ActorSystemBuilder` 等にツールボックス選択 API と `StdActorSystem` エイリアスを追加し、`actor-std` の再エクスポートおよびドキュメントを更新する。
5. 変更範囲のテスト群と `./scripts/ci-check.sh all` を実行し、回帰がないことを確認する。

## Risks & Mitigations
- **ジェネリクス露出による複雑化**: 型エイリアスと `actor-std` の再エクスポートで典型ケースをカバーし、拡張利用時にのみ型パラメータを意識させる道筋を用意する。
- **GAT 導入による推論失敗**: 主要 API で明示的な型記述が不要になるようテストを追加し、回避手段（型エイリアス）をドキュメント化する。

## Impacted APIs / Modules
- `modules/utils-core`（`SyncMutexFamily` と `RuntimeToolbox` の定義）
- `modules/actor-core`（`ActorSystemState`, `Mailbox` などミューテックス生成箇所、およびビルダー API）
- `modules/actor-std`（`StdToolbox` 再エクスポートとサンプル更新）

## References
- 現行 `ActorRuntimeMutex` 実装
- `SyncMutexLike` と `SpinSyncMutex` / `StdSyncMutex`
