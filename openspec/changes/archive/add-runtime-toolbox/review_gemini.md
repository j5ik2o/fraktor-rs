# `add-runtime-toolbox` レビュー (Gemini)

## レビューサマリー

この提案は、`ActorRuntimeMutex` のバックエンドが暗黙的に切り替わる問題を解決し、アプリケーション側から同期プリミティブの挙動を明示的に選択できるようにするために `RuntimeToolbox` 抽象を導入するものです。これにより、`std` と `embedded` 環境での異なる同期プリミティブの提供が可能になり、将来的な拡張性も向上します。

**主な変更点:**
- `RuntimeToolbox` トレイトの定義と、`SyncMutex` などの同期プリミティブを生成するインターフェースの提供。
- `NoStdToolbox` / `StdToolbox` などの標準環境の実装。
- `ActorSystemBuilder` / `ActorSystemConfig` への `RuntimeToolbox` 設定 API の追加。
- ランタイム内部のロック生成を `RuntimeToolbox` 経由にリファクタリング。
- ドキュメントとサンプルの更新。

**影響:**
- `ActorSystem` 初期化 API にオプションが追加されるが、デフォルトは互換性を維持。
- 利用者がバックエンドを明示的に選択できるようになり、制御性が向上。
- 将来的な同期プリミティブの追加が容易になる。

**スコープ:**
- `RuntimeToolbox` 抽象と標準環境の実装。
- `ActorSystemBuilder` / `ActorSystemConfig` への環境設定 API 導入。
- ランタイム内部の同期プリミティブ生成を `RuntimeToolbox` 経由に統一。
- `actor-std` から `StdToolbox` の再エクスポートとドキュメント更新。

**非スコープ:**
- `ActorSystem<R>` のような完全ジェネリクス化。
- `RuntimeToolbox` の実行時切替。
- `Condvar` 等の新たな同期プリミティブの実装。

**ロールアウト計画:**
1. 既存コードの調査。
2. `RuntimeToolbox` トレイトと標準環境の実装、単体テスト。
3. `ActorSystemBuilder` / `ActorSystemConfig` への環境設定 API 追加、デフォルト設定。
4. ランタイム内部のロック生成を `RuntimeToolbox` 経由に書き換え。
5. `actor-std` から `StdToolbox` の再エクスポート、サンプル・ドキュメント更新。

**リスクと軽減策:**
- **動的ディスパッチによるオーバーヘッド**: ロック生成時のみのため影響は軽微。
- **API 増加による複雑化**: デフォルトを保持し、ガイドで利用方法を明記。

**影響を受ける API / モジュール:**
- `modules/utils-core`
- `modules/actor-core`
- `modules/actor-std`

**タスク:**
1. 調査: `ActorRuntimeMutex::new` および `SpinSyncMutex::new` の呼び出し箇所、`ActorSystemBuilder` / `ActorSystemConfig` の初期化フロー。
2. 実装: `RuntimeToolbox` トレイトと標準実装、`ActorSystemBuilder` / `ActorSystemConfig` への環境設定 API、ランタイム内部のリファクタリング。
3. 検証・ドキュメント: ユニットテスト、`actor-std` から `StdToolbox` の再エクスポート、ガイド・サンプル更新、CI チェック。

## 全体的な評価

この提案は、`cellactor-rs` の設計を改善し、より柔軟で拡張性の高いランタイム環境を提供する上で非常に有益であると考えられます。特に、`std` と `embedded` 環境での同期プリミティブの選択肢を明示的に提供できる点は、多様なユースケースに対応するために重要です。リスクと軽減策も適切に考慮されており、段階的なロールアウト計画も現実的です。

## 改善点・質問

- `NoStdToolbox` がデフォルトとして設定されるとのことですが、`actor-std` を利用するユーザーにとっては `StdToolbox` がデフォルトとなる方が自然かもしれません。`actor-std` クレートの `Cargo.toml` で `StdToolbox` をデフォルトフィーチャーとして有効にするなどの考慮はありますか？
- `RuntimeToolbox` トレイトの具体的なメソッドシグネチャや、`SyncMutex` 以外の同期プリミティブ（例: `RwLock`、`Condvar`）をどのように扱うかについて、もう少し詳細な設計があると良いかもしれません。
- `ActorSystem<R>` のような完全ジェネリクス化が非ゴールとされていますが、将来的に検討する可能性はありますか？もしそうであれば、今回の設計がその将来的なジェネリクス化を妨げないか、あるいは促進するような考慮がされているかを確認したいです。
