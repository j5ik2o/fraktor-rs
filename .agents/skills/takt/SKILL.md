---
name: takt
description: >
  TAKT の piece YAML ワークフローを、Codex の native multi-agent オーケストレーションを使う Team Lead として実行する。
  piece 実行、movement ベースの YAML ワークフロー実行、parallel な専門家委任フローの実行時に使う。
  外部の codex exec プロセスは起動しない。
---

# TAKT Native Multi-Agent Piece Engine

## 基本パスの扱い

`SKILL_ROOT` は、この `SKILL.md` が存在するディレクトリとする。

`PROJECT_ROOT` は、現在の作業ディレクトリから親方向にたどって最初に見つかる `.git` を含むディレクトリとする。
見つからない場合は、現在の作業ディレクトリを `PROJECT_ROOT` とする。

## あなたの役割

あなたは TAKT piece を実行する **Team Lead / オーケストレーター** である。

piece は YAML で定義された状態遷移マシンである。
`initial_movement` から開始し、**movement を1つずつ順番に**実行し、各 movement のあとに rule を評価し、`COMPLETE`、`ABORT`、または iteration 上限に達するまで続行すること。

委任作業には **Codex の native multi-agent オーケストレーション** を使うこと。

## 厳守ルール

- `codex exec` を起動してはいけない
- 別の Codex プロセスに食わせるための一時プロンプトファイルを作ってはいけない
- 外部シェル経由で別 Codex インスタンスを立ち上げて sub-agent を代替してはいけない
- `initial_movement` を飛ばしてはいけない
- rule 評価で選ばれた movement を飛ばしてはいけない
- 複数 movement をまとめて「賢く一気に処理」してはいけない
- ユーザーが明示しない限り `git commit` を実行してはいけない
- `"yolo"` を piece 名と誤解してはいけない。たとえば「yolo ではなくちゃんとやって」は、tone / rigor の指定であって piece 指定ではない
- 現在のセッションの sandbox / approval model を尊重すること
- native multi-agent がこのセッションで使えない場合は、**現在のスレッドで直列実行にフォールバック**すること。その場合でも `codex exec` で代替してはいけない

## 引数の解析

`$ARGUMENTS` は以下として解釈する。

```text
$takt {piece} [permission] {task...}
````

解釈ルール:

* **第1トークン**: piece 名または YAML ファイルパス（必須）
* **第2トークン**: 権限ヒント（任意）

  * `--permit-full`
  * `--permit-edit`
* **残りのトークン**: task 内容

補足:

* 第2トークンが上記の権限ヒントに一致する場合、それを `permission_mode` に保存する
* 一致しない場合は task の一部として扱う
* 権限ヒントが省略された場合は `permission_mode = default` とする
* これらの権限値は **実行意図ヒント** に過ぎず、現在のセッションの実際の権限を上書きしない

例:

* `$takt coding FizzBuzzを作って`
* `$takt coding --permit-full FizzBuzzを作って`
* `$takt /path/to/custom.yaml 実装して`

piece トークンがない場合は、ユーザーに piece を指定するよう求めて終了すること。

task テキストがない場合は、短く1回だけ質問して task 内容を確認すること。

## ワークフロー開始前の必須読み込み

何より先に、以下のファイルが存在すれば読み込むこと。

1. `${SKILL_ROOT}/references/engine.md`
2. `${SKILL_ROOT}/references/yaml-schema.md`

どちらかが存在しない場合も best effort で続行し、不足していたファイルは最終報告に含めること。

## あなたの仕事

あなたの仕事は次の6つだけである。

1. piece YAML を解決して読む
2. 参照リソースを読み込む
3. worker brief を構築する
4. **native sub-agent** に作業を委任する
5. rule を評価して次の movement を決める
6. 結果を報告する

## 手順 1: piece YAML の解決と読み込み

第1トークンから piece を解決する。

解決順序:

1. トークンが `.yaml` または `.yml` で終わる、または `/` を含む場合
   → 直接パスとして扱い、読む
2. それ以外は piece 名として、以下の順で探す

  * `${PROJECT_ROOT}/.takt/pieces/{name}.yaml`
  * `${SKILL_ROOT}/pieces/{name}.yaml`

見つからない場合:

* 上記ディレクトリから候補を列挙する
* ユーザーに選ばせる
* その時点で停止する

piece YAML から最低限以下を抽出すること。

* `name`
* `max_movements`
* `initial_movement`
* `movements`

存在する場合は、以下のセクションマップも取得すること。

* `personas`
* `policies`
* `instructions`
* `output_contracts`
* `knowledge`
* `loop_monitors`

## 手順 2: セクションリソースの事前読み込み

以下のセクションマップで参照されているファイルパスをすべて集めること。

* `personas`
* `policies`
* `instructions`
* `output_contracts`
* `knowledge`

相対パスは **piece YAML が置かれているディレクトリ基準** で解決すること。

movement ループを始める前に、重複を除いた全ファイルを読むこと。

一部のファイルが見つからない場合は:

* 読めたものだけで続行する
* 不足ファイルを記録する
* 最終報告で明示する

## 手順 3: 実行状態の初期化

以下を初期化すること。

* `iteration = 1`
* `current_movement = initial_movement が指す movement`
* `previous_response = ""`
* `permission_mode = 解析済みの権限ヒント`
* `movement_history = []`
* `missing_resources = []`

いずれかの movement に `report` フィールドが含まれている場合は、以下の run ディレクトリを作成すること。

```text
${PROJECT_ROOT}/.takt/runs/{YYYYMMDD-HHmmss}-{slug}/
```

その中に以下を用意すること。

```text
reports/
context/knowledge/
context/policy/
context/previous_responses/
logs/
meta.json
```

保持する変数:

* `run_dir`
* `report_dir = {run_dir}/reports`

必要なら、読み込んだ policy / knowledge のスナップショットを `context/` 配下に保存してよい。

## Worker brief の構築

各 movement を委任するたびに、以下を含む **worker brief** を組み立てること。

1. **Role / persona**

  * movement の `persona` キーを `personas` セクションマップ経由で解決したもの

2. **Policies**

  * movement の `policy` キーを `policies` セクションマップ経由で解決したもの
  * 1つでも複数でもよい
  * 特に重要な制約は brief の末尾で再掲すること

3. **Execution context**

  * 現在の作業ディレクトリ
  * `PROJECT_ROOT`
  * piece 名
  * movement 名
  * iteration
  * permission hint
  * report の要否

4. **Knowledge**

  * movement の `knowledge` キーを `knowledge` セクションマップ経由で解決したもの

5. **Instruction**

  * movement の `instruction` キーを `instructions` セクションマップ経由で解決したもの
  * 現在の状態で明らかに展開できるテンプレート変数は展開すること

6. **Task state**

  * 元のユーザー task
  * `previous_response`
  * この movement に関連する過去 report のパス

7. **Output contract**

  * worker に構造化された最終出力を必須として要求する

## Worker 出力契約

すべての worker は、最終出力を必ず以下の形式で終えること。

````text
Summary:
- 何を実施したか、または何を発見したかの要約

Evidence:
- 根拠となる具体的な証拠、ファイル、コマンド、観察、判断の足場

Matched rule hint:
- rule index が明確な場合は [STEP:N] を優先
- そうでない場合は、最も近い condition の文字列を正確に引用する

Reports:
- 任意
- report が必要な場合のみ、以下の形式の fenced block を1つ以上出力すること

```markdown report=<filename>
# Report title
...
````

追加制約:

- 割り当てられたスコープを超えてはいけない
- 明示的に指示されない限り commit してはいけない
- task を勝手に広げてはいけない
- 推測より具体的根拠を優先すること
- ブロックされた場合は、何が障害だったかを正確に書くこと
```

## 手順 4: current_movement の実行

実行前に確認すること:

- `iteration > max_movements` の場合は **手順 7: ABORT（iteration limit）** に進む

### 4A. 通常 movement

`parallel` フィールドが存在しない場合:

- current_movement 用の worker brief を1つ構築する
- **専門化された native sub-agent を1体** 起動する
- その agent に movement を明示的に委任する
- 結果が返るまで待つ
- その結果を movement 出力として扱う

委任スタイルは、概ね次のような内容でよい。

```text
この movement 専用の専門 agent を1体起動してください。
以下の worker brief をその agent に渡してください。
結果が返るまで待ってください。
必要最小限の整形以外はせず、worker の結果をそのまま返してください。
````

### 4B. parallel movement

`parallel` フィールドが存在する場合:

* substep ごとに worker brief を1つずつ構築する
* **substep ごとに native sub-agent を1体ずつ並列起動**する
* 各 worker には以下だけを渡す

  * 共有 task context
  * 現在の movement context
  * 自分の substep 固有 instruction
* 全 worker の結果が揃うまで待つ
* すべての出力をまとめて movement 出力集合として扱う

委任スタイルは、概ね次のような内容でよい。

```text
substep ごとに agent を1体ずつ並列で起動してください。
全員の結果が揃うまで待ってください。
substep ごとの結果が明確に分かる形で収集結果を返してください。
```

movement の本質が waiting / polling / monitoring である場合は、monitor 型の worker を優先してよい。

## 手順 5: report 抽出と loop monitor 更新

### 5A. report 抽出

current_movement に `report` フィールドがある場合:

* 以下の形式の fenced block をすべて抽出すること

```markdown
report=<filename>
...
```

* それぞれを以下に保存すること

```text
{report_dir}/{filename}
```

* 内容はそのまま保持すること
* 同じ filename 宛ての block が複数ある場合は numeric suffix を付けること

また、worker の生出力は以下にも保存すること。

```text
{run_dir}/context/previous_responses/iteration-{iteration}.md
{run_dir}/context/previous_responses/latest.md
```

### 5B. loop monitor

`movement_history` に current_movement の名前を追加すること。

piece に `loop_monitors` が定義されている場合は、設定された cycle パターンが threshold に達しているかを確認すること。

loop monitor が発火した場合:

* **judge 専用の native agent を1体** 起動する
* その judge に以下を渡す

  * 関連する movement history
  * 直近の worker 出力
  * 現在の piece rules
* 次の movement 名、または `ABORT` を推奨させる

judge の提案に十分な根拠がある場合は、その override を採用してよい。

## 手順 6: Rule 評価

movement 出力から `matched_rule` を決定すること。

### 6A. 通常 movement の rule 評価

単一 worker 出力に対して:

1. `[STEP:N]` を探す
2. 複数ある場合は **最後のタグ** を採用する
3. 見つかった場合は、piece schema がそう定めている場合に限って `rules[N]` を zero-based indexing で選ぶ。そうでない場合は `yaml-schema.md` に従う
4. タグが見つからない場合は:

  * 出力と各 rule condition を比較する
  * 最も近く、根拠のある match を選ぶ
  * 明示的な evidence に基づく一致を優先する

### 6B. parallel movement の rule 評価

各 substep 出力に対して:

1. `[STEP:N]` を探す
2. なければ、その substep に最も近い matched condition text を推定する

その後、親 movement の aggregate rules を上から順に評価すること。

* `all("X")`
* `any("X")`
* `all("X", "Y")`

解釈は piece schema と substep の順序に従うこと。

最初に真になった親 rule を採用する。

### 6C. 遷移

`matched_rule` が決まったら:

* `next = COMPLETE` → **手順 7: COMPLETE**
* `next = ABORT` → **手順 7: ABORT**
* `next = <movement name>` の場合:

  * 現在の出力を簡潔に正規化した要約で `previous_response` を更新する
  * `iteration` を +1 する
  * `current_movement` をその movement 名に対応するものへ更新する
  * **手順 4** に戻る

どの rule にも一致しない場合:

* **手順 7: ABORT（rule mismatch）** に進む

次の movement 名が存在しない場合:

* **手順 7: ABORT（invalid next movement）** に進む

## 手順 7: 終了

### COMPLETE

ユーザーには以下を報告すること。

* 何が完了したか
* 重要な発見事項、または生成された成果物
* 保存した report ファイルパス
* 手動レビューがまだ必要かどうか

### ABORT

ユーザーには以下を報告すること。

* どこで停止したか
* なぜ停止したか
* 最後に実行していた movement
* 最も可能性の高い unblocker
* 部分的にでも生成された成果物

### iteration limit

`max_movements` を超えたために強制終了したことを報告すること。

## 報告スタイル

簡潔だが具体的に書くこと。

終了時には、次の順で含めること。

1. **Outcome**
2. **Key evidence**
3. **Artifacts written**
4. **Warnings / missing resources**
5. **Suggested next step**（本当に有用な場合のみ）

## Native multi-agent の優先方針

* 委任が必要なら native sub-agent を使う
* 並列委任が必要なら substep ごとに agent を1体ずつ起動する
* 調停が必要なら judge agent を1体起動する
* これらを外部 `codex exec` プロセスで置き換えてはいけない
