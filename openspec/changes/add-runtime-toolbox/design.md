# RuntimeToolbox設計メモ

## 目的
- ランタイム固有の同期プリミティブ（初期段階では `SyncMutex`）を注入するための単一の入口を用意する。
- アクターランタイムAPI全体へジェネリクスを伝播させない。
- 環境が指定されない場合は現行の `SpinSyncMutex` ベース実装と同一の挙動を維持する。

## RuntimeToolboxトレイト
```
pub trait RuntimeToolbox: Send + Sync + 'static {
    type SyncMutex<T>: SyncMutexLike<T> + Send + Sync + 'static;

    fn make_mutex<T>(&self, value: T) -> Self::SyncMutex<T>;
}
```
- 将来的なプリミティブ（例:`RwLock`, `Condvar`）は関連型とコンストラクタメソッドを追加することで拡張できる。
- 実装はスレッド安全である必要がある。`ActorSystemState` で共有インスタンスを保持し複数スレッドからアクセスするため `Send + Sync` が必須。

## 標準環境
- `NoStdToolbox`: すべてのビルドで利用可能で、`SpinSyncMutex` を使用し `no_std` ターゲットとの互換性を保つ。
- `StdToolbox`: `std` feature 有効時のみコンパイルされ、内部的に `StdSyncMutex` を利用する。
- `actor-std` `StdToolbox` を再エクスポートし、ヘルパー的なビルダーを提供できる。

## 所有モデル
- `ActorSystemBuilder` は `with_runtime_toolbox` で `ArcShared<dyn RuntimeToolbox>` を受け取る。
- 利用者が環境を指定しない場合はビルダーが共有の `ArcShared<NoStdToolbox>` インスタンスを挿入する。
- `ActorSystemState` が環境を保持し、他のサブシステムは `state.runtime_toolbox().make_mutex(...)` を呼び出してミューテックスを取得する。

## 検討した代替案
- 完全なジェネリクス（`ActorSystem<R: RuntimeToolbox>`）: 利用者向けAPIすべてにジェネリクスを強制し型推論を複雑化させるため却下。
- Mutex専用ファクトリ: 将来のプリミティブをカバーできず、複数の注入ポイントが残るため却下。

## 性能上の考慮
- `ArcShared<dyn RuntimeToolbox>` を使う場合、ミューテックス生成時に動的ディスパッチが1回発生するが、メッセージパッシングに比べ生成頻度が低いため許容できる。
- プロファイルでオーバーヘッドが無視できない場合は、今後の作業でジェネリック特化や環境専用アクターを導入可能。

## 将来拡張
- 追加のプリミティブ（例:`type RwLock<T>`, `type Condvar`）は関連型と対応するコンストラクタを追加することで対応できる。
- 他の実行コンテキスト（組込み、決定的スケジューリング）向け環境も既存コードに影響を与えずに `RuntimeToolbox` を実装できる。
