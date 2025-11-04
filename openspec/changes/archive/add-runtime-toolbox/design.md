# RuntimeToolbox設計メモ

## 目的
- ミューテックス生成処理を `SyncMutexFamily` という専用トレイトにまとめ、`RuntimeToolbox` からはファミリーを選択するだけに簡素化する。
- 公開 API（`ActorSystem`, `ActorRuntimeMutex` など）は既存シグネチャを維持し、内部的にのみツールボックスの差し替えを可能にする。
- 将来追加する同期プリミティブについても、同様の *Family* パターンで拡張できる土台を用意する。

## RuntimeToolbox / SyncMutexFamily
```
pub trait SyncMutexFamily {
    type Mutex<T>: SyncMutexLike<T> + Send + 'static;
    fn create<T: Send>(value: T) -> Self::Mutex<T>;
}

pub trait RuntimeToolbox {
    type MutexFamily: SyncMutexFamily;
}
```
- ミューテックス生成は `SyncMutexFamily::create` に統一し、`Spin`/`Std` など環境ごとの差異はファミリー実装で吸収する。
- `RuntimeToolbox` は関連型で利用するファミリーを指し示すだけの軽量トレイトとし、将来的に `RwLockFamily` 等を追加して拡張する。
- コンパイル時にモノモーフィックなコードを生成できるため、動的ディスパッチは不要。

## 標準環境
- `SpinMutexFamily`（`SpinSyncMutex` ベース）と `StdMutexFamily`（`StdSyncMutex` ベース）を実装し、それぞれを `NoStdToolbox` / `StdToolbox` から公開する。
- `actor-std` は `StdToolbox` と、内部ジェネリクスを包んだ `StdActorSystem` / `StdActorSystemBuilder` などのエイリアスを再エクスポートする。
- カスタム環境は独自のファミリーとツールボックスを実装し、ビルダー API 経由で注入する。

## 所有モデル
- 内部実装では `ActorSystemState<TB>` などが `TB: RuntimeToolbox` を保持し、必要な同期プリミティブを `TB::MutexFamily::create` で生成する。
- 公開 API は `ActorSystem` / `ActorSystemBuilder` といった既存型を維持し、内部フィールドに選択済みツールボックス（ジェネリクスもしくは enum）を保持する。ユーザは `StdActorSystem` エイリアスかビルダーの `with_toolbox::<StdToolbox>()` を呼ぶだけで良い。
- `ActorRuntimeMutex<T>` は既定の `NoStdToolbox` を指す型エイリアスのまま残し、内部コードでは `ToolboxMutex<T, TB>` のような別名を使用してバックエンドを切り替える。

## 検討した代替案
- **動的ディスパッチ**: トレイトオブジェクト経由で生成する案は、`Send`/`Sync` 境界と割り込み安全性の検証コストが高く、パフォーマンス低下も懸念されるため棄却。
- **ジェネリクス全面公開**: すべての公開型に `<TB>` を持たせる案はユーザの認知負荷が高く、エイリアスだけで隠蔽しきれないため棄却。
- **差し替えマクロ**: マクロ展開で環境ごとの差を吸収する案は、プリミティブ追加のたびにマクロの分岐が肥大化するため棄却。

## 性能および型推論
- `SyncMutexFamily::create` はインライン化可能な薄いラッパーであり、既存の `SpinSyncMutex::new` 呼び出しと同等のコストに収まる。
- ユーザコードは既存 API のままコンパイルされるため、型推論に追加負荷は掛からない。内部でのみ `TB` ジェネリクスが現れるが、エイリアス経由で容易に確認できるようにする。

## 将来拡張
- `SyncMutexFamily` と同様のパターンで `RwLockFamily` や `CondvarFamily` を追加すれば、ランタイム全体の構造を崩さずに拡張できる。
- ツールボックス実装を増やすだけで、RTOS やハードリアルタイム環境など任意のバックエンドを注入できる。
