1. [ ] `TypedAskResponseGeneric<R, TB>` と typed future ラッパーを設計し、reply handle・future を型安全に公開する
2. [ ] 既存の `TypedActorRefGeneric::ask` をジェネリック化して `TypedAskResponseGeneric<R, TB>` を返すよう改変する
3. [ ] 旧 `AskResponseGeneric` 依存部分を除去し、新 API を内部実装へ全面適用する
4. [ ] typed ask の happy path / 型不一致エラー / `R` 制約違反などの単体テストを追加する
5. [ ] example または doc コメントで typed ask の利用手順を説明し、`./scripts/ci-check.sh all` を通す
