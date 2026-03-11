# instruction_template の使用禁止

TAKT ピース（`.takt/pieces/*.yaml`）において `instruction_template` を使用してはならない。

## 理由

`instruction` と `instruction_template` を併記した場合、**`instruction_template` だけが有効**になり、`instruction` に指定した facet の内容は完全に無視される。
また、`instruction_template` は廃止予定（deprecated）のフィールドである。

## 禁止パターン

```yaml
# ❌ WRONG: instruction_template が優先され、instruction は無視される
- name: implement
  instruction: implement          # この内容は無視される
  instruction_template: |
    プロジェクト固有の追加指示
```

## 正しい方法

**`instruction` のみを使用し、プロジェクト固有の指示を含んだ instruction ファイルを作成する。**

Built-in instruction の内容 + プロジェクト固有の追加指示をひとつのファイルにまとめ、`instructions:` セクションで登録する。

```yaml
# ✅ CORRECT: instruction ファイルにすべての指示をまとめる
instructions:
  my-implement: ../facets/instructions/my-implement.md  # built-in + 追加指示を統合

movements:
  - name: implement
    instruction: my-implement    # 単独で使う
```

## instruction ファイルの作成手順

1. 対応する built-in instruction の内容をコピーする
   - パス例: `$(npm root -g)/takt/builtins/ja/facets/instructions/implement.md`
2. プロジェクト固有の追加指示を末尾に追記する
3. `.takt/facets/instructions/` に保存し、ピース YAML の `instructions:` セクションに登録する

## 適用範囲

- `.takt/pieces/*.yaml` の全ムーブメント
- 既存ピースも含めて `instruction_template` を使用しているものは修正すること
