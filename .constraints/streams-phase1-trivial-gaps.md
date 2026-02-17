---
status: done
---
## 意図
streams モジュールの trivial ギャップ6件を既存APIの組み合わせ・委譲で実装する

## 制約 (MUST)
- 新規 `StageKind` バリアントの追加不可（既存の組み合わせのみ）
- `./scripts/ci-check.sh all` がエラーなしで通る
- 既存テストが全て Pass する
- TDD: テストを先に書き、テストが通る最小限の実装を行う
- Codex設計レビュー: 実装前に Codex Architect で設計レビューを実施する
- Codexコードレビュー: 実装完了後に Codex Code Reviewer でレビューを実施する

## 非目標 (NOT)
- Attributes 型システムの導入
- BidiFlow への Mat 型パラメータ追加（Phase 2）
- `KillSwitch` trait の抽出（既存具象型で十分）
- オペレーターカタログ（`DefaultOperatorCatalog`）への登録

## タスク
- [x] Codex設計レビュー実施・指摘対応
- [x] `ClosedShape` / `Flow::from_function` / `named` のテスト作成（Red）
- [x] `ClosedShape` / `Flow::from_function` / `named` の実装（Green）
- [x] `KillSwitches::shared/single` のテスト作成（Red）
- [x] `KillSwitches::shared/single` の実装（Green）
- [x] `BidiFlow::identity/reversed` のテスト作成（Red）
- [x] `BidiFlow::identity/reversed` の実装（Green）
- [x] Codexコードレビュー実施・指摘対応

## 受入テスト
- `Flow::from_function(|x| x + 1)` で要素が変換される
- `ClosedShape` が型エイリアスとしてコンパイルできる
- `KillSwitches::shared()` が `SharedKillSwitch` を返す
- `BidiFlow::identity()` を通したストリームが要素を変更せず通過させる
- `BidiFlow::reversed()` の戻り型で top/bottom が入れ替わっている

## 設計判断
- `Flow::from_function(f)`: `Flow::new().map(f)` への委譲（静的メソッド）
- `named(name)`: `const fn` で `self` を返す no-op（`extrapolate` と同パターン）。Attributes 未導入のため名前は保持しない
- `ClosedShape`: `pub type ClosedShape = ()` の型エイリアス。`core/shape/closed_shape.rs` に配置
- `KillSwitches`: 新規ファイル `core/lifecycle/kill_switches.rs` にファクトリ関数を配置。`SharedKillSwitch::new()` / `UniqueKillSwitch::new()` への委譲（引数なし。現在の SharedKillSwitch::new() は name を取らない）
- `BidiFlow::identity<T>()`: `BidiFlow<T, T, T, T>` を返す。`BidiFlow { top: Flow::new(), bottom: Flow::new() }` で構築
- `BidiFlow::reversed(self)`: `BidiFlow<InBottom, OutBottom, InTop, OutTop>` を返す。top/bottom フィールドを入れ替え
