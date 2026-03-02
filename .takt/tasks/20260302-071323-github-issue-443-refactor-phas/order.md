## GitHub Issue #443: refactor: Phase C — cluster/remote/persistence/streams モジュールから TB: RuntimeToolbox を除去する

## 親 Issue

#412 の Phase C に相当する。Phase A は #441 で完了済み。Phase B の完了が前提。

## 背景

Phase B で actor モジュールから TB を除去した後、残りのモジュール（cluster, remote, persistence, streams）からも同様に除去する。

## 現状（各モジュール）

| モジュール | Generic 型を含むファイル数 | TB bounds を含むファイル数 |
|---|---|---|
| cluster | 49 | 42 |
| remote | 31 | 28 |
| persistence | 26 | 20 |
| streams | 8 | 6 |

## 実施内容

各モジュールについて:
- [ ] 全 `XxxGeneric<TB>` 型から `TB` パラメータを除去し、`Xxx` にリネーム
- [ ] `Generic` サフィックスを全廃
- [ ] `pub type Xxx = XxxGeneric<NoStdToolbox>` エイリアスを削除
- [ ] `std/` 内の型エイリアス再エクスポートを削除（アダプター実装は残す）

## 推奨順序

依存の少ないモジュールから順に:
1. **streams** （8ファイル、最小）
2. **persistence** （26ファイル）
3. **remote** （31ファイル）
4. **cluster** （49ファイル、actor 依存が多い）

## 完了条件

- cluster/remote/persistence/streams の全モジュールで `TB: RuntimeToolbox` bounds が0
- 全モジュールで `Generic` サフィックス型が0
- `./scripts/ci-check.sh all` がパスする

## 関連

- 親 Issue: #412
- Phase A 完了: #441
- Phase B: #442

### Labels
refactoring