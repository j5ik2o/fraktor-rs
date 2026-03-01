## GitHub Issue #412: refactor: TB: RuntimeToolbox 型パラメータを feature flag に置換し、Generic 二重構造を廃止する

## 背景
fraktor-rs の `TB: RuntimeToolbox` 型パラメータは実質ロック実装（spin vs std）の選択のみを抽象化している。
これはビルド時に確定する選択であり、feature flag（条件コンパイル）で解決すべき問題。
170型に TB が伝播し、~64型に Generic サフィックス、~60個の NoStd エイリアス、~29個の Std エイリアスが発生している。

## 実施計画

### Phase A: utils の RuntimeMutex/RuntimeRwLock 導入
- `RuntimeMutex<T>` / `RuntimeRwLock<T>` 型エイリアスを feature flag で定義
- `SyncMutexFamily` / `SyncRwLockFamily` の Family パターンを廃止
- `ToolboxMutex<T, TB>` / `ToolboxRwLock<T, TB>` を `RuntimeMutex<T>` / `RuntimeRwLock<T>` に置換

### Phase B: actor モジュールから TB を除去
- 全 `XxxGeneric<TB>` 型から `TB` パラメータを除去し、`Xxx` にリネーム
- `Generic` サフィックスを全廃
- `pub type Xxx = XxxGeneric<NoStdToolbox>` エイリアス ~60個を削除
- `std/` 内の型エイリアス再エクスポート ~29個を削除（アダプター実装は残す）

### Phase C: 他モジュール（cluster/remote/persistence/streams）から TB を除去
- 同様に TB パラメータ除去とリネーム

### Phase D: RuntimeToolbox trait の廃止
- `RuntimeToolbox` trait を削除
- `NoStdToolbox` / `StdToolbox` 構造体を削除
- Clock を必要に応じて `Arc<dyn MonotonicClock>` で Scheduler に注入
- `core/` + `std/` ディレクトリ構造は Port & Adapter として維持

## 追加注意事項
- `tick_source()` メソッドの再設計が必要（ライフタイム問題）
- `SyncMutexFamily::create()` → `RuntimeMutex::new()` への一括置換
- TB を直接使う型は37%（30型）のみ、残り63%は伝播のみ
- Dylint の8つの lint が TB パターンを前提にしている可能性あり

## 完了条件
- 全 Phase (A→D) が完了し、TB パラメータが完全に除去されている
- `./scripts/ci-check.sh all` がパスする
- Generic サフィックス型が0になっている
