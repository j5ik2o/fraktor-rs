# actor モジュール ギャップ分析

この文書は actor モジュールの **現在の残ギャップ** を把握するための入口である。
過去版の更新履歴と詳細な突合根拠は別ファイルへ分離した。

- 更新履歴: [actor-gap-analysis-history.md](./actor-gap-analysis-history.md)
- 詳細根拠: [actor-gap-analysis-evidence.md](./actor-gap-analysis-evidence.md)

## 前提

- Pekko 互換仕様の実現と Rust らしい設計の両立を目指す。
- 型名・関数名・シグネチャの存在だけでは「実装済み」と判定しない。
- 状態遷移、失敗経路、監視/再起動、panic 変換、mailbox 契約まで Pekko の意味論と一致して初めて完了とみなす。
- Java DSL、JVM reflection、HOCON dynamic loading、Pekko IO は actor core の parity 対象外とする。
- modules/*-core のコアロジックは原則 no_std とし、std 依存は modules/*-adaptor-std に置く。

## 現在の結論

2026-04-24 時点の第22版では、actor モジュールの主要公開 API ギャップは 0 件である。
第8版で検出した内部セマンティクスギャップは high 0 / medium 1 まで縮小しており、残る medium は remote / cluster 連携待ちの DeathWatch 1 件に絞られている。

## 現在サマリー

| 指標 | 現在値 |
|------|--------|
| 分析版 | 第22版 |
| 分析日 | 2026-04-24 |
| Pekko 公開型数（parity 対象） | 101 |
| fraktor-rs 対応実装数 | 101 |
| 公開 API カバレッジ | 101/101 = 100% |
| 公開 API ギャップ | 0 |
| 内部セマンティクス high | 0 |
| 内部セマンティクス medium | 1 |
| 内部セマンティクス low | 約 11 |
| 構造改善候補 | 1 |

## 残存ギャップ

| ID | 種別 | 状態 | 次アクション |
|----|------|------|--------------|
| AC-M4b | DeathWatch の AddressTerminated 購読 | remote / cluster 完成まで deferred | remote / cluster transport の障害通知経路が確定した後、watched remote actor へ Terminated 相当を配送する |
| classic kernel public surface | 内部補助型の公開範囲縮小 | 継続的な構造改善 | 利用者向け facade から再公開される型を基準に、内部補助型を `pub(crate)` へ寄せる |

AC-M4b は現在の唯一の medium parity gap である。
classic kernel public surface は保守性改善であり、Pekko 実行時契約の blocker ではない。

## カバレッジ

| 層 | Pekko 対応数 | fraktor-rs 実装数 | カバレッジ |
|----|--------------|-------------------|------------|
| core / untyped kernel | 39 | 39 | 100% |
| core / typed wrapper | 56 | 56 | 100% |
| std / adaptor | 6 | 6 | 100% |
| 合計 | 101 | 101 | 100% |

## 完全カバー済みカテゴリ

| カテゴリ | 状態 |
|----------|------|
| classic actor core | 16/16 |
| supervision / fault handling | 8/8 |
| typed core surface | 36/36 |
| dispatch / mailbox | 13/13 |
| event / logging | 10/10 |
| pattern | 5/5 |
| classic routing | 15/15 |
| typed routing | 7/7 |
| discovery / receptionist | 9/9 |
| scheduling / timers | 8/8 |
| ref / resolution | 6/6 |
| delivery / pubsub | 8/8 |
| serialization | 8/8 |
| extension | 4/4 |
| coordinated shutdown | 5/5 |
| std adaptor | 6/6 |

## 現在の注意点

### AC-M4b

Pekko の `DeathWatch.scala` は remote address termination を EventStream 経由で購読し、remote node 障害時に watched actors へ Terminated を配送する。
fraktor-rs では remote / cluster transport の障害通知モデルがまだ actor core の DeathWatch と完全接続されていないため、AC-M4b は deferred として残す。

完了条件:

- remote / cluster 側で address terminated 相当のイベントが actor core へ届く。
- `watch` / `watch_with` で登録済みの remote target に対して Terminated 相当が一度だけ配送される。
- `unwatch` 後の late notification が抑止される。
- local DeathWatch の重複チェックと terminated dedup を壊さない。

### MB-M3

Pekko bounded mailbox は `pushTimeOut` により producer を blocking するが、fraktor-rs は async Rust の設計上、bounded overflow を non-blocking な `MailboxOverflowStrategy` で扱う。
これは parity カウント対象外の design divergence として扱う。
必要性が実運用で出た場合のみ、別 change で `MailboxOverflowStrategy::Fail { timeout }` などを含む大規模再設計として検討する。

### ES-M1

Pekko EventStream は lock-free CAS ベースだが、fraktor-rs は `SharedRwLock<EventStream>` を使う。
機能差ではなく性能差のため low として扱う。
高頻度 subscribe / unsubscribe が実測で問題化した場合のみ再検討する。

## 次アクション

1. remote / cluster transport の障害通知経路が固まったら AC-M4b を change 化する。
2. actor core の公開型を棚卸しし、利用者向けでない補助型を `pub(crate)` 化する。
3. mailbox producer backpressure は実測またはユーザー要求が出るまで再設計しない。

## 参照

- 詳細な内部セマンティクス比較: [actor-gap-analysis-evidence.md](./actor-gap-analysis-evidence.md)
- 第1版から第22版までの更新履歴: [actor-gap-analysis-history.md](./actor-gap-analysis-history.md)
