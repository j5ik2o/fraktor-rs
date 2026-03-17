# 実装判断ログ

## 主要判断
| 項目 | 判断 | 理由 |
|------|------|------|
| `new_with_clock` のシグネチャ設計 | `Clock` trait オブジェクトを `ArcShared` で受け取る | 既存の `Clock` trait が `ArcShared<dyn Clock>` で使用されるパターンに合わせた。テストで `FakeClock` を注入可能にするため |
| 既存 `new` コンストラクタの変更 | 内部で `SystemClock` をデフォルトとして使用するよう変更 | 後方互換性を維持しつつ、`new_with_clock` との実装共有を実現 |
| tokio `test-util` feature の追加方法 | `dev-dependencies` の tokio features に `test-util` を追加 | `start_paused = true` と `tokio::time::pause()` がテストで必要。本番コードには影響しない |
| `scripts/ci-check.sh` の unit/integration テスト分離 | `cargo test --lib` と `cargo test --test` を分離実行 | integration テストが tokio runtime 設定の違いで unit テストと干渉するのを防止 |
| テストファイルの修正有無 | 修正なし | write_tests ムーブメントで作成済みのテストがそのままパスしたため、変更不要 |

## 影響とトレードオフ
- `new_with_clock` の追加は公開 API の拡張であり、既存の `new` の動作は変更なし（破壊的変更なし）
- `Clock` trait への依存が `CircuitBreaker` / `CircuitBreakerShared` に追加されるが、デフォルトは `SystemClock` なので既存利用者への影響はゼロ
- ci-check.sh の分離により CI 実行時間がわずかに増加するが、テストの信頼性が向上する