# RuntimeToolbox設計メモ

## 目的
- `RuntimeToolbox` を型レベルのマッピングとして再設計し、動的ディスパッチを排除する。
- `ActorSystem` の表層 API は従来通り維持しつつ、内部実装を `ActorSystemGeneric<TB>` へ移行する。
- `SyncMutex` 以外のプリミティブを追加しやすい土台（GAT）の上で拡張性を確保する。

## RuntimeToolbox トレイト
```
pub trait RuntimeToolbox {
    type SyncMutex<T>: SyncMutexLike<T> + Send + 'static;
    // 将来: type RwLock<T>, type Condvar などを追加
}
```
- 実行時に値を生成するメソッドは提供せず、各関連型が持つ `SyncMutexLike::new` を直接利用する。
- トレイト自体はオブジェクトセーフ性を気にせず設計でき、ジェネリクス経由でコンパイル時に解決される。
- `Send + Sync` 制約は関連型に付与し、`RuntimeToolbox` 自身はマーカー的役割に留める。

## 標準環境
- `NoStdToolbox`: `SpinSyncMutex` を関連型として公開。
- `StdToolbox`: `StdSyncMutex` を関連型として公開。`actor-std` が `pub use StdToolbox` と `pub type StdActorSystem = ActorSystemGeneric<StdToolbox>` を提供する。
- カスタム環境は `RuntimeToolbox` を実装し、利用側で型引数として指定する。

## 所有モデル
- `ActorSystemGeneric<TB>` / `ActorSystemState<TB>` など主要構造体は `TB: RuntimeToolbox` を型パラメータとして保持する。
- 既存 API を壊さないため `pub type ActorSystem = ActorSystemGeneric<NoStdToolbox>` のような型エイリアスを導入する。
- ビルダーも `ActorSystemBuilder<TB>` とし、`ActorSystemBuilder::<StdToolbox>::new()` のように利用する。典型ケースではエイリアスを通じてジェネリクス記法を意識させない。

## 検討した代替案
- **動的ディスパッチ**: オブジェクトセーフ性の制約により `make_sync_mutex` が実現困難で、実装複雑化＆パフォーマンス低下リスクが高いため棄却。
- **環境ごとの差し替えマクロ**: メンテナンス性が低く、将来のプリミティブ追加でマクロ改修が必要になるため棄却。

## 性能および型推論
- すべてがコンパイル時に解決されるため、ランタイムコストは現行と同等。
- 型推論失敗に備え、主要 API で `TB` を明示できる補助型エイリアス（`StdActorSystem` など）を提供する。

## 将来拡張
- `RuntimeToolbox` に `type RwLock<T>` `type Condvar` などを追加するだけで、新しい同期プリミティブを全経路で利用可能になる。
- `RuntimeToolbox` 実装を追加するだけで別環境（RTOS・デターミニスティック実行など）を組み込める。
