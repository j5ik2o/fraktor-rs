# タスク仕様

## 目的

`.takt/pieces/pekko-porting.yaml` に不足している loop_monitors を追加し、無限ループを防止する。

## 要件

- [ ] `reviewers → fix → reviewers` サイクルに loop_monitor を追加する（threshold: 3）
- [ ] `supervise → plan` サイクルに loop_monitor を追加する（threshold: 2）
- [ ] `ai_fix ↔ ai_no_fix` サイクルに loop_monitor を追加する（threshold: 3）
- [ ] 各 loop_monitor に supervisor ペルソナによる judge と ABORT 条件を設定する

## 受け入れ基準

- 3つのサイクルすべてに loop_monitors が設定されている
- 非生産的なループが検出された場合に適切な遷移先（ABORT または別ムーブメント）が設定されている
- `validate-takt-files.sh` がエラーなしで通る

## 参考情報

- GitHub Issue: #140（#141, #142 を統合済み）
- 対象ファイル: `.takt/pieces/pekko-porting.yaml`
- 既存の loop_monitor: `ai_review ↔ ai_fix` サイクル（threshold: 3）
