```markdown
# {name} モジュール ギャップ分析（統合レポート）

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | N |
| fraktor-rs 公開型数 | M |
| カバレッジ（型単位） | M/N (XX%) |
| インターフェース一致率 | Y/M (YY%) |
| 設計等価率 | Z/比較対象数 (ZZ%) |
| ギャップ数 | G |

## カテゴリ別ギャップ

### {カテゴリ名}

| Pekko API | Pekko参照 | fraktor対応 | Layer | 難易度 | YAGNI | 備考 |
|-----------|-----------|-------------|-------|--------|-------|------|
| `symbolName` | `File.scala:Lnn` | 未対応 / `file.rs:Symbol` | 型/IF/設計 | trivial/easy/medium/hard/n/a | 必要/不要 | 説明 |

## 整合性注記

Layer 間で矛盾がある場合はここに記載する（矛盾がなければ「なし」）。

## 実装優先度の提案

### Phase 1: trivial
- ...

### Phase 2: easy
- ...

### Phase 3: medium
- ...

### Phase 4: hard
- ...

### 対象外（n/a / YAGNI不要）
- ...
```
