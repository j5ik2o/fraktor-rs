## Why
`ActorRuntimeMutex` は利用クレートに応じて暗黙的にバックエンドが切り替わるが、アプリケーション側からは挙動を明示できず学習コストが高い。今後 `std` / `embedded` で異なる同期プリミティブを提供したい場合、共通の注入ポイントが必要になる。`RuntimeToolbox` 抽象を導入し、ランタイムが利用する同期プリミティブを一括して管理できるようにすることで、利用者は明示的にバックエンドを選択でき、将来的な拡張にも備えられる。

## What Changes
- `RuntimeToolbox` トレイトを再定義し、Generic Associated Type（GAT）で `type SyncMutex<T>` を提供するだけのマッピングに切り替える。
- `NoStdToolbox` / `StdToolbox` など標準環境を実装し、`actor-std` から `StdToolbox` と `ActorSystemGeneric<StdToolbox>` の型エイリアスを公開する。
- `ActorSystem` を `ActorSystemGeneric<TB>` へリファクタリングし、既存 API 互換のため `type ActorSystem = ActorSystemGeneric<NoStdToolbox>` を提供する。`ActorSystemBuilder` なども同様にツールボックスをジェネリクス引数で受け取る形に再構成する。
- ランタイム内部で `ActorRuntimeMutex` を型エイリアスに置き換え、各構造体で `TB::SyncMutex<T>::new` を利用するスタイルへ変更する。
- ドキュメントとサンプルを更新し、利用者が `StdToolbox` などを型引数として選択する方法を示す。

## Impact
- 既存の `ActorSystem` / `ActorRuntimeMutex` 呼び出しは型エイリアスで互換性を保ちつつ、追加の型引数を指定するだけで異なる同期バックエンドを選択できるようになる。
- ランタイム内部から動的ディスパッチを排除し、コンパイル時最適化を維持したまま複数環境を表現できる。
- 将来的に `Condvar` などを追加する際は `RuntimeToolbox` の関連型を拡張すれば良く、構造体のフィールド定義や生成関数はジェネリクス経由で自然に対応できる。

## Scope
### Goals
1. `RuntimeToolbox` 抽象と標準環境実装を GAT ベースで整備する。
2. `ActorSystemGeneric<TB>` / `ActorSystemBuilder<TB>` などのジェネリクス API を追加し、既存の型エイリアスで後方互換を維持する。
3. ランタイム内部の同期プリミティブ参照を `TB::SyncMutex<T>` 経由に置換する。
4. `actor-std` から `StdToolbox` と `StdActorSystem`（仮称）を再エクスポートし、ドキュメントとサンプルを更新する。

### Non-Goals
- ランタイム起動後の `RuntimeToolbox` 動的変更。
- `Condvar` 等の新たな同期プリミティブ本体の実装（別提案で扱う）。
- `actor-core` の API を全面的なジェネリクス公開に切り替える（型エイリアスでラップする範囲に留める）。

## Rollout Plan
1. `ActorRuntimeMutex::new` を直接呼び出している箇所を洗い出し、GAT で置き換える対象をリスト化する。
2. `RuntimeToolbox` / `NoStdToolbox` / `StdToolbox` を GAT に合わせて実装し、ユニットテストでミューテックス生成を確認する。
3. `ActorSystemGeneric<TB>` と関連ビルダーを導入し、既存 API とのエイリアス互換を整備する。
4. ランタイム内部のロック生成呼び出しを `TB::SyncMutex<T>::new` 形式へ段階的にリプレースする。
5. `actor-std` の再エクスポート・サンプル・ガイドを更新し、型パラメータで環境を選択するフローを提示する。

## Risks & Mitigations
- **ジェネリクス露出による複雑化**: 型エイリアスと `actor-std` の再エクスポートで典型ケースをカバーし、拡張利用時にのみ型パラメータを意識させる道筋を用意する。
- **GAT 導入による推論失敗**: 主要 API で明示的な型記述が不要になるようテストを追加し、回避手段（型エイリアス）をドキュメント化する。

## Impacted APIs / Modules
- `modules/utils-core`（`RuntimeToolbox` の定義）
- `modules/actor-core`（`ActorSystemBuilder`, `ActorSystemState`, 各モジュール内の同期プリミティブ生成）
- `modules/actor-std`（`StdToolbox` 再エクスポートとサンプル更新）

## References
- 現行 `ActorRuntimeMutex` 実装
- `SyncMutexLike` と `SpinSyncMutex` / `StdSyncMutex`
