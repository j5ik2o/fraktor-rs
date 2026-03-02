## GitHub Issue #442: refactor: Phase B — actor モジュールから TB: RuntimeToolbox を除去し Generic サフィックスを廃止する

## 親 Issue

#412 の Phase B に相当する。Phase A は #441 で完了済み。

## 背景

Phase A で `RuntimeMutex<T>` / `RuntimeRwLock<T>` の feature flag ベース型エイリアスが導入され、`SyncMutexFamily` / `SyncRwLockFamily` の Family パターンは廃止済み。

次のステップとして、最大のモジュールである **actor** から `TB: RuntimeToolbox` 型パラメータを除去する。

## 現状（actor モジュール）

- Generic 型を含むファイル: **202 ファイル**
- TB bounds を含むファイル: **175 ファイル**
- `pub type Xxx = XxxGeneric<NoStdToolbox>` エイリアス: 多数
- `std/` 内の `pub type Xxx = XxxGeneric<StdToolbox>` 再エクスポート: 多数

## 実施内容

- [ ] 全 `XxxGeneric<TB>` 型から `TB` パラメータを除去し、`Xxx` にリネーム
- [ ] `Generic` サフィックスを全廃
- [ ] `pub type Xxx = XxxGeneric<NoStdToolbox>` エイリアスを削除
- [ ] `std/` 内の型エイリアス再エクスポートを削除（アダプター実装は残す）
- [ ] `ToolboxMutex` / `ToolboxRwLock` の残存があれば `RuntimeMutex` / `RuntimeRwLock` に置換

## 作業のコツ

- TB を直接使う型（`RuntimeMutex` / `RuntimeRwLock` を保持）は全体の約37%。残り63%は TB を下位に伝播するだけ
- **伝播専用の型から先に TB を除去する**と効率的
- 各ステップでコンパイルが通る状態を維持すること

## 完了条件

- actor モジュール内に `TB: RuntimeToolbox` bounds が0になっている
- actor モジュール内に `Generic` サフィックス型が0になっている
- `./scripts/ci-check.sh all` がパスする

## 関連

- 親 Issue: #412
- Phase A 完了: #441

### Labels
refactoring