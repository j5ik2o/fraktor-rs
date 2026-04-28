# E2E テスト配置整理と Pekko 由来カバレッジ拡充計画

## 概要

fraktor-rs の E2E テストを「crate-local」と「cross-crate」に分ける。各クレートの public API だけで閉じる E2E は既存どおり `modules/*/tests/` に残し、複数クレートをまたぐ runtime 全体の E2E だけを `tests/e2e` ワークスペースクレートへ集約する。その後、Pekko `actor-tests/src/test` を参考に actor runtime の不足 E2E を追加する。

## 主要方針

- `tests/e2e` を workspace member として追加する。
- `tests/e2e` は cross-crate E2E 専用 crate とし、`actor-core`, `actor-adaptor-std`, 必要に応じて `remote-*`, `cluster-*`, `persistence-*` を依存として持つ。
- crate-local E2E は移動しない。例: `modules/actor-core/tests/classic_user_flow_e2e.rs`, `typed_user_flow_e2e.rs`, `actor_path_e2e.rs`。
- std adaptor 単体の E2E は `modules/actor-adaptor-std/tests/` に残す。例: `std_adaptor_boot_e2e.rs`。
- cross-crate 性質が明確なものだけを `tests/e2e/tests/*.rs` へ新規追加または移動する。

## 移行手順

1. 既存 E2E / integration test を棚卸しし、`crate-local`, `std-adaptor-local`, `cross-crate` に分類する。
2. `tests/e2e/Cargo.toml` を作成し、workspace member に追加する。
3. `tests/e2e/tests/actor_runtime_boot.rs` の最小 smoke E2E を追加し、test crate の依存・feature・CI 経路を確認する。
4. 既存テストのうち cross-crate と判断できるものだけを `tests/e2e` へ移す。判断が曖昧なものは crate-local に残す。
5. Pekko actor-tests 由来の不足 E2E を優先度順に追加する。

## Pekko 由来で追加する E2E

- P0: `actor_selection`。local hierarchy 上の relative/absolute selection、non-existing path、wildcard/fan-out、`Identify` / `ActorIdentity` を public API で検証する。
- P0: `actor_lifecycle`。spawn -> failure/restart -> child termination -> stop の lifecycle ordering を ActorSystem 経由で検証する。
- P0: `death_watch`。watch/unwatch/watch_with、既に停止済み actor、重複通知抑止、parent-child termination を public API で検証する。
- P1: `stash_and_timer`。stash/unstash の restart/stop 時挙動、timer cancel on restart/stop、ReceiveTimeout と `NotInfluenceReceiveTimeout` を検証する。
- P1: `dispatcher_mailbox`。std executor と mailbox policy を含むため `tests/e2e` または `actor-adaptor-std/tests` に置き、priority/control-aware/throughput の observable ordering を検証する。
- P2: `event_stream_and_logging`。dead letters、suppressed dead letters、event stream subscription/unsubscription、std tracing bridge を検証する。
- P2: `coordinated_shutdown`。ActorSystem terminate から coordinated shutdown が一度だけ走ること、phase order、reason 固定を検証する。

## テスト計画

- 変更中は対象 crate 単位で `rtk cargo test -p ... --test ...` を実行する。
- `tests/e2e` 追加後は `rtk cargo test -p fraktor-e2e-tests` 相当で単体実行できるようにする。
- 最終的にソースを編集した場合は `rtk ./scripts/ci-check.sh ai all` を実行し、完了まで待つ。
- CI 対象に `tests/e2e` が含まれていない場合は、既存 `scripts/ci-check.sh` の integration-test 対象へ追加する。

## 前提

- `tests/e2e` は production crate ではなく publish 対象外の workspace test crate とする。
- crate-local public API だけで閉じる E2E は移動しない。
- `actor-core` が `actor-adaptor-std` に依存する方向は作らない。cross-crate E2E 側が両方へ依存する。
- Pekko testkit 互換 API は今回の対象外。必要なら別計画で扱う。
- remote / cluster / persistence を含む E2E は、actor 単体 E2E の後続フェーズで追加する。
