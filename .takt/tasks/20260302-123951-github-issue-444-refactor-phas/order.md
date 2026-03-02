## GitHub Issue #444: refactor: Phase D — RuntimeToolbox trait / NoStdToolbox / StdToolbox を廃止する

## 親 Issue

#412 の Phase D に相当する。Phase A は #441 で完了済み。Phase B・C の完了が前提。

## 背景

Phase B・C で全モジュールから `TB: RuntimeToolbox` 型パラメータが除去された後、`RuntimeToolbox` trait 自体と関連構造体を廃止する。

## 実施内容

- [ ] `RuntimeToolbox` trait を削除
- [ ] `NoStdToolbox` 構造体を削除
- [ ] `StdToolbox` 構造体を削除
- [ ] Clock を必要に応じて `Arc<dyn MonotonicClock>` で Scheduler に注入する形に再設計
- [ ] `tick_source()` メソッドの再設計（現在 `RuntimeToolbox::tick_source(&self)` にライフタイムでスコープされている）
- [ ] `core/` + `std/` ディレクトリ構造は Port & Adapter として**維持する**（削除しない）
- [ ] utils モジュール内の不要になった RuntimeToolbox 関連コードを整理

## tick_source() の再設計について

現在の設計:
```rust
RuntimeToolbox::tick_source(&self) -> SchedulerTickHandle<'_>
```

TB 廃止後は `SchedulerTickHandle` の生成元を別途設計する必要がある。候補:
- Scheduler に直接保持させる
- 独立したファクトリ関数にする

## 完了条件

- `RuntimeToolbox` trait がコードベースから完全に消えている
- `NoStdToolbox` / `StdToolbox` が消えている
- Clock の注入が正しく動作している
- `./scripts/ci-check.sh all` がパスする

## 関連

- 親 Issue: #412
- Phase A 完了: #441
- Phase B: #442
- Phase C: #443

### Labels
refactoring