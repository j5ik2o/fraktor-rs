## プロジェクト原則（全 change 共通）

本 change は以下 4 原則に従って設計される:

1. **Pekko 互換仕様の実現 + Rust らしい設計**: Pekko の lifecycle / supervision / death watch の意味論を再現しつつ、Rust の所有権と mailbox 実行モデルに合わせて翻訳する
2. **手間が掛かっても本質的な設計を選ぶ**: 同期 dispatcher だけを黙らせる局所 workaround ではなく、restart completion の正しい scheduling 境界を定義する
3. **フォールバックや後方互換性を保つコードを書かない**: 正式リリース前のため、暫定 API や互換層は増やさず、必要なら破壊的変更を許容する
4. **no_std core + std adaptor 分離**: `modules/actor-core` の kernel / mailbox / dispatcher で完結させ、std 固有の都合を core に漏らさない

## Why

`2026-04-20-pekko-restart-completion` で restart completion の 2 フェーズ化は入ったが、**default `pre_restart` が `stop_all_children()` を呼ぶ経路**はまだ Pekko parity に達していない。

現状の fraktor-rs では同期 dispatcher / inline executor 環境で親が `SystemMessage::Recreate(cause)` を処理すると、

1. `pre_restart` が `stop_all_children()` を呼ぶ
2. child への `SystemMessage::Stop` がその場で実行される
3. child 停止に伴う `DeathWatchNotification(child_pid)` も同じ mailbox run 中に親へ戻る
4. `handle_death_watch_notification` が即座に `finish_recreate(cause)` を実行する

という流れになり、`fault_recreate()` の「子あり restart は child termination まで deferred する」という契約が、同一 dispatch turn の中で潰れてしまう。

このズレは ignored テスト `al_h1_t2_default_pre_restart_stops_children_and_defers_finish_recreate` が既に示している。問題は `stop_all_children` 単体ではなく、**restart completion をどの mailbox turn / scheduling boundary で完了させるか** という kernel 契約にあるため、既存 change から切り離した follow-up change として扱う。

## What Changes

- `default pre_restart` 経路において、**child stop の enqueue** と **restart completion の実行** が同一 inline drain で潰れないよう、mailbox / dispatcher / actor kernel の scheduling 境界を見直す
- `fault_recreate -> handle_death_watch_notification -> finish_recreate` の責務分割を再確認し、**同期 dispatcher でも「子あり restart は deferred」と言える条件**を明文化する
- ignored テスト `al_h1_t2_default_pre_restart_stops_children_and_defers_finish_recreate` を pass させることを change の受け入れ条件にする
- 必要に応じて、以下のいずれかを設計対象に含める
  - restart path 専用の self-system-message による turn 分離
  - mailbox system queue drain の再入制御
  - `stop_all_children` の「mark / unwatch / queue stop」段階と、実 stop 実行段階の分離
  - `finish_recreate` を同一 drain 中に実行しない明示的な completion boundary

## Capabilities

### Modified Capabilities
- `pekko-restart-completion`: default `pre_restart` を含む全 restart 経路で、同期 dispatcher でも Pekko と同じ deferred completion 契約を満たすようにする
- `actor-runtime-safety`: restart 中の mailbox suspend / child termination / completion 実行順序を、実際の dispatch turn 境界まで含めて定義する

## Impact

- 対象は `modules/actor-core` に限定される見込み
  - `core/kernel/actor/actor_cell.rs`
  - `core/kernel/actor/actor_context.rs`
  - `core/kernel/dispatch/mailbox/*`
  - `core/kernel/dispatch/dispatcher/*`
- 主な影響は restart completion の内部 scheduling であり、public API 変更は必須ではない。ただし correctness のために内部 system message や helper の追加・整理は許容する
- `2026-04-20-pekko-restart-completion` の parity claim を本当に成立させる最後の詰めになる

## Non-goals

- panic guard や lifecycle hook 全般の例外処理は扱わない
- remote / cluster の death watch 転送は扱わない
- typed 側の reason 伝播拡張は扱わない
- mailbox / dispatcher の一般的な性能最適化は目的にしない

## Dependencies

- **前提**: `2026-04-20-pekko-restart-completion`
- 本 change はその follow-up であり、既存 restart completion change の設計意図を保ったまま、同期 dispatcher で破れている deferred completion 契約を補完する

## Artifact 方針

- 今回は `proposal.md` のみ先行作成する
- `design.md` と `tasks.md` は、実装に着手する直前に current branch / failing tests / mailbox 実装差分を再確認してから起こす
