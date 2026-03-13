## Context

この change は、Open な `[BugBot]` と `[CodeRabbit:major]` issue を「指摘単位」ではなく「根本原因単位」でまとめて解消するためのものです。対象は主に 3 系統あります。

1. `modules/actor` の runtime 安全性
mailbox policy と queue の真実源不整合、bounded queue の競合、supervision 再起動時の behavior 初期化、stash の lock 下コールバック、group router の route 戦略表現など、no_std core の不変条件が揺らいでいます。

2. `modules/streams` の backpressure / terminal 整合
`Source::create` の同期寄り収集、`SourceQueue` の pending/wake、`actor_ref` / `actor_ref_with_backpressure` の契約、timer/async callback の apply failure 時の消失など、キューと state machine の境界が曖昧です。

3. `.takt` と CI の整合
piece YAML の構造破損、未参照 instruction、nested code fence、AI モードでガードされない `cargo` 実行経路により、ワークフロー解釈と実行経路が壊れています。

制約:
- `Less is more` と `YAGNI` を守り、過剰な抽象化を避ける
- `modules/*/src/core` と `std` の依存方向を崩さない
- 後方互換性は要求されないため、誤解を招く API 名や公開範囲は必要なら破壊的に是正する
- 最後は `./scripts/ci-check.sh ai all` で検証可能な状態にする

## Goals / Non-Goals

**Goals:**
- actor runtime の queue/policy/supervision/router/stash 周りの不変条件を回復する
- streams の backpressure、wake、timer/apply failure、terminal 状態遷移を観測可能かつ再現可能な契約に揃える
- `.takt` / CI を、機械的に解釈可能で一貫した実行経路を持つ状態へ修正する
- 各 root cause に対する回帰テストを追加し、同種の bot issue 再発を防ぐ

**Non-Goals:**
- 既存 issue をすべて 1 PR で機械的にクローズすること自体は目的にしない
- streams や actor の新機能追加は行わない
- Pekko 完全互換の新設計をこの change だけで完了させることは目指さない
- `.takt` 全体の全面再設計や workflow 再編は行わない
- serena 関連の Git URL pinning 変更（#748 系）は、この change では扱わない

## Decisions

### 1. issue ではなく root-cause バッチで実装する

個別 issue ごとに場当たり修正すると、mailbox や source queue のような共有部位で差分が競合します。そこで実装単位は以下の 3 バッチに固定します。

- actor runtime safety
- streams backpressure integrity
- workflow integrity

これにより、同じファイルに集まる複数 issue を 1 回の設計変更でまとめて解消できます。

代替案:
- issue ごとに順番対応する
  却下理由: 同一シンボルに対して別の仮修正が重なり、再修正コストが増えるため

### 2. actor mailbox は「policy と queue が一致する構築経路」だけを許可する

`Mailbox::new_with_queue` が公開のままだと、policy と queue の不整合を型外から持ち込めます。actor 側では registry / selector / props が関与するため、policy と queue の真実源を 1 つに揃える必要があります。

採用方針:
- `new_with_queue` は crate 内に閉じる、もしくは整合検証込みの生成に絞る
- registry 経由生成では resolve 済み `mailbox_config.policy()` を唯一の真実源にする
- bounded queue の同期は queue 内部で完結させ、外側の TOCTOU を許さない
- stash / mailbox の user callback は lock 解放後に評価する

代替案:
- 公開 API のままドキュメントだけで整合条件を課す
  却下理由: bot 指摘の対象が不変条件なので、利用者規約では弱すぎるため

### 3. supervision / behavior / router は「名前より契約」を優先して是正する

`intercept_behavior` の one-shot slot は restart と相性が悪く、`ConsistentHash` も現在の `hash % N` 実装と名前が釣り合っていません。後方互換不要の前提を使い、契約に合わせて API または実装を是正します。

採用方針:
- supervised restart 後も behavior interceptor が再初期化可能な状態遷移にする
- route 戦略は、真に一貫性のある hashing を実装するか、保証水準に合わせて命名を弱める
- 例コードも無期限待機や panic-on-reply を避ける

代替案:
- 既存 API 名を維持して内部実装だけ最小修正する
  却下理由: 誤解を招く名前を残すと再度同種 issue が発生するため

### 4. streams は queue state machine を明示し、drain 後の値消失を禁止する

streams の issue は、`offer/poll/wake/complete/fail/apply` の責務が分散し、途中失敗時の値保持戦略が不明確な点に集中しています。

採用方針:
- `Source::create` は遅い producer を前提にした非同期取り込み契約へ寄せる
- `SourceQueue` 系は pending offer 数、wake 通知、complete/cancel の遷移を明文化する
- `on_async_callback` / `on_timer` の出力は apply 成否と独立に失われないバッファリングへ寄せる
- `actor_sink` / `actor_ref_with_backpressure` は名前どおりの delivery / ack 契約を持たせる

代替案:
- 個別に sleep や retry を増やして暫定回避する
  却下理由: WouldBlock や lost output の根本原因を隠すだけで、契約は改善されないため

### 5. workflow 系は「構造妥当性」と「一貫した実行経路」を最小ルールで固定する

`.takt` と設定文書の issue は、記法の揺れで機械解釈が壊れていることが原因です。

採用方針:
- piece YAML は sibling/indent 構造を正しい schema に戻す
- output contract の nested code fence は競合しない記法へ変える
- 未参照 instruction は wiring するか削除する
- `scripts/ci-check.sh` の `cargo` 実行は `run_cargo` に集約する

代替案:
- bot 指摘箇所だけ最小修正する
  却下理由: 同種の記法崩れや unguarded 経路が他所に残りやすいため

## Risks / Trade-offs

- [actor API を破壊的に是正する] → 既存テストと examples を同時更新し、命名と契約の不一致を残さない
- [streams state machine の変更で既存テストが大量に崩れる] → まず契約テストを先に追加し、状態遷移ごとに修正する
- [workflow 修正が人間運用に影響する] → `.takt` は schema を満たす最小差分に留め、CI スクリプトも既存運用を崩さない範囲で統一する
- [issue 数が多く一度に広がりすぎる] → 実装順を actor → streams → workflow に固定し、各バッチ完了ごとに回帰確認する

## Migration Plan

1. proposal で定義した capability ごとに spec を作成する
2. actor runtime safety を先に実装し、mailbox / supervision / router の回帰テストを通す
3. streams backpressure integrity を実装し、source queue / actor sink / timer 系の回帰テストを通す
4. workflow integrity を実装し、`.takt` と `scripts/ci-check.sh` の構造検証を行う
5. 変更全体に対して `./scripts/ci-check.sh ai all` を実行する

ロールバック:
- 正式リリース前のため機能フラグによる併存は行わず、必要なら change 単位で revert する

## Open Questions

- router の `ConsistentHash` は rendezvous hashing に強化するか、より弱い名前へ改名するか
- `pekko-gap-analyze` は wiring で残すか、不要物として削除するか
- `.takt` の未参照 instruction は wiring 優先か削除優先か
