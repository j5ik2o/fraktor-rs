---
status: done
---
## 意図
Flow/Source の no-op スタブオペレーター 7 種を、既存オペレーターの組み合わせで実装する

## 制約 (MUST)
- 既存の `map`, `stateful_map`, `wire_tap`, `buffer` 等の組み合わせのみで実装する（新規 StageKind 追加不可）
- Flow と Source の両方に同一セマンティクスで提供する（Sink は対象外）
- `./scripts/ci-check.sh all` がエラーなしで通る
- 既存テストが全て Pass する
- オペレーターカタログ（`DefaultOperatorCatalog`）への登録は行わない（既存 57 件を維持）

## 非目標 (NOT)
- 新規 `StageKind` バリアントの追加
- `GraphStageLogic` のライフサイクルフック拡張（`do_on_cancel` は対象外）
- `OperatorKey` / `OperatorCoverage` への追加
- Sink への `fold` / `reduce` 追加（既に実装済み）
- `log` クレートや `tracing` クレートの新規依存追加

## タスク
- [x] `detach`: `async_boundary()` 委譲（Flow + Source）
- [x] `log` / `log_with_marker`: `wire_tap(|_| {})` で実ステージ化（Flow のみ）
- [x] `do_on_first`: `wire_tap` + `fired` フラグでコールバック呼び出し（Flow のみ）
- [x] `conflate`: `map(|v| v)` でパススルー（Flow のみ）
- [x] `fold`（Flow 上）: `scan(initial, func).drop(1)` で実装（Flow + Source）
- [x] `reduce`（Flow 上）: `scan(None, ...).drop(1).flatten_optional()` で実装（Flow + Source）
- [x] 各オペレーターの単体テスト追加

## 受入テスト
- `detach` を通したストリームが要素を落とさず順序を保つ
- `log` を通したストリームが要素を変更せず通過させる
- `do_on_first` のコールバックが最初の要素でのみ呼ばれる
- `fold` が全要素をアキュムレータで畳み込み、最終値のみ emit する
- `reduce` が `fold` と同等の結果を返す（初期値なし、最初の要素が seed）

## 設計判断
- `detach`: `buffer(1)` は同期実行モデルでオーバーフローするため `async_boundary()` に変更
- `conflate`: `buffer(1, DropOldest)` は同期実行モデルで要素が落ちるため `map(|v| v)` に変更
- `log`: no_std + `deny(cfg_std_forbid)` のため実際のログ出力は行わない
- `fold`: Pekko の「完了時のみ emit」とは異なり、`scan` + `drop(1)` で running accumulation を emit
- `reduce`: `stateful_map` の二重クロージャ問題を回避するため `scan` + `flatten_optional` に変更
