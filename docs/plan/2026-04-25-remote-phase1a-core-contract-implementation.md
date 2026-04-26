# remote Phase 1A core 契約実装計画

## 目的

Pekko remote 互換の Phase 1A として、std runtime 配線を伴わない core/no_std 側の契約を実装する。
対象は `ThrowableNotSerializableException` 相当、`PhiAccrualFailureDetector` の監視先 address 束縛、`DeadlineFailureDetector`、`RemoteLogMarker` の 4 件に限定する。

## 実装対象

| 項目 | 実装先 | 方針 |
|------|--------|------|
| `ThrowableNotSerializableException` 相当 | `actor-core` serialization | 元例外の message と class name を保持する payload 型として公開する |
| `PhiAccrualFailureDetector` address 対応 | `remote-core` failure_detector | 既存の phi 計算を変えず、生成時に監視先 address を束縛できる constructor を追加する |
| `DeadlineFailureDetector` | `remote-core` failure_detector | 明示 `now_ms` を受け取り、Pekko と同じ排他的 deadline 境界で判定する |
| `RemoteLogMarker` | `remote-core` instrument | Pekko と同じ marker 名・property key を持つ `ActorLogMarker` を返す |

## 実装順序

1. 既存の serialization / failure detector / instrument の module wiring とテスト配置を確認する。
2. `actor-core` serialization payload を 1 公開型 1 ファイルで追加し、既存テストを通す。
3. `remote-core` failure detector の Phi address 対応と deadline detector を追加する。
4. `remote-core` instrument の `RemoteLogMarker` を追加する。
5. `cargo fmt`、対象 clippy、対象テスト、`./scripts/ci-check.sh ai dylint` を実行して結果を `.takt` レポートへ記録する。

## スコープ外

`advertised addresses / listen event`、`lifecycle event publishing from association effects`、remote actor ref 生成、payload serialization、Artery framing、DeathWatch 統合は Phase 1A では実装しない。
