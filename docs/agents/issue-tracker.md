# Issue tracker: GitHub

このリポジトリの issue と PRD は GitHub Issues で管理する。操作には `gh` CLI を使う。

## 運用ルール

- issue を作る: `gh issue create --title "..." --body "..."`。複数行本文は heredoc を使う。
- issue を読む: `gh issue view <number> --comments`。必要に応じて `jq` でコメントとラベルを絞り込む。
- issue を一覧する: `gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'` に、必要な `--label` / `--state` フィルタを加える。
- コメントする: `gh issue comment <number> --body "..."`
- ラベルを付け外しする: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- close する: `gh issue close <number> --comment "..."`

repo は `git remote -v` から推定する。通常、clone 内で `gh` を実行すれば自動で対象 repo が選ばれる。

## skill が "publish to the issue tracker" と指示した場合

GitHub issue を作成する。

## skill が "fetch the relevant ticket" と指示した場合

`gh issue view <number> --comments` を実行する。
