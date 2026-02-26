# {領域名}バグスキャンレポート

## 結果

- CLEAN / BUGS_FOUND
- 重大度: Critical=0 / High=0

## 検出（BUGS_FOUND）

|finding_id|重大度|種類|場所|問題|修正|
|---|---|---|---|---|---|
|{PREFIX}-NEW-{file}-L{line}|Critical/High|{種類}|`{file}:{line}`|{説明}|{対応}|

## 継続指摘 / 解消

- 継続: `{PREFIX}-PERSIST-{file}-L{line}` / `{file}:{line}` → `{file}:{line}`
- 解消: `{PREFIX}-RESOLVED-{file}-L{line}` / {確認内容}

## スキャン範囲

- {モジュール・ファイル}

## Finding 種別

- `LOGIC` / `SEC` / `CONC`
