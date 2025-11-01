## Why
`ActorRuntimeMutex` は利用クレートに応じて暗黙的にバックエンドが切り替わるが、アプリケーション側からは挙動を明示できず学習コストが高い。今後 `std` / `embedded` で異なる同期プリミティブを提供したい場合、共通の注入ポイントが必要になる。`RuntimeToolbox` 抽象を導入し、ランタイムが利用する同期プリミティブを一括して管理できるようにすることで、利用者は明示的にバックエンドを選択でき、将来的な拡張にも備えられる。

## What Changes
- `RuntimeToolbox` トレイトを定義し、`SyncMutex` などの同期プリミティブを生成するインターフェースを提供する。
- `NoStdToolbox` / `StdToolbox` など標準環境を実装し、`actor-std` から `StdToolbox` を再エクスポートする。
- `ActorSystemBuilder` / `ActorSystemConfig` に `RuntimeToolbox` を設定できる API を追加し、未指定時は `NoStdToolbox` を利用する。
- ランタイム内部のロック生成を `RuntimeToolbox` 経由にリファクタリングし、`ActorSystemState` 等の中心状態が環境を保持して各コンポーネントへ提供する。
- ドキュメントとサンプルを更新し、利用者が環境を明示選択する手順を示す。

## Impact
- `ActorSystem` 初期化 API に新たなオプションが追加されるが、デフォルトは従来通り `SpinSyncMutex` を利用するため互換性は維持される。
- 利用者がバックエンドを明示的に選択できるようになり、 std / embedded など複数の構成が混在するプロジェクトでの制御性が向上する。
- 将来的に `Condvar` 等の同期プリミティブを追加する際にも、`RuntimeToolbox` に拡張を施すだけで対応しやすくなる。

## Scope
### Goals
1. `RuntimeToolbox` 抽象と標準環境実装を追加する。
2. `ActorSystemBuilder` / `ActorSystemConfig` に環境設定 API を導入する。
3. ランタイム内部の同期プリミティブ生成を `RuntimeToolbox` 経由に統一する。
4. `actor-std` から `StdToolbox` を再エクスポートし、サンプルとガイドを更新する。

### Non-Goals
- `ActorSystem<R>` のような完全ジェネリクス化。
- `RuntimeToolbox` の実行時切替（起動後の動的変更）。
- `Condvar` 等新たな同期プリミティブの実装（別提案で扱う）。

## Rollout Plan
1. 既存コードで `ActorRuntimeMutex::new` を利用している箇所を調査し、環境注入が必要な範囲を把握する。
2. `RuntimeToolbox` トレイトと標準環境を実装し、単体テストで `SyncMutex` が生成できることを確認する。
3. `ActorSystemBuilder` / `ActorSystemConfig` に環境設定 API を追加し、デフォルトを `NoStdToolbox` に設定する。
4. ランタイム内部のロック生成を `RuntimeToolbox` 経由に書き換え、環境を `ActorSystemState` 等で一元管理する。
5. `actor-std` から `StdToolbox` を再エクスポートし、サンプル・ドキュメントを更新する。

## Risks & Mitigations
- **動的ディスパッチによるオーバーヘッド**: 環境注入はロック生成時のみ呼ばれるため影響は軽微。必要に応じてジェネリクス化を後続検討とする。
- **API 増加による複雑化**: デフォルトを保持しつつ、ガイドで利用方法を明記することで緩和する。

## Impacted APIs / Modules
- `modules/utils-core`（`RuntimeToolbox` の定義）
- `modules/actor-core`（`ActorSystemBuilder`, `ActorSystemState`, 各モジュール内の同期プリミティブ生成）
- `modules/actor-std`（`StdToolbox` 再エクスポートとサンプル更新）

## References
- 現行 `ActorRuntimeMutex` 実装
- `SyncMutexLike` と `SpinSyncMutex` / `StdSyncMutex`
