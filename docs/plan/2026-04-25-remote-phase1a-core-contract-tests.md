# remote Phase 1A core 契約テスト計画

## 目的

Pekko remote 互換の Phase 1A として、std runtime 配線を伴わない core/no_std 側の契約を先に固定する。
対象は `ThrowableNotSerializableException` 相当、`FailureDetectorWithAddress`、`DeadlineFailureDetector`、`RemoteLogMarker` の 4 件とする。

## テスト対象

| 項目 | 追加予定の振る舞い | テスト観点 |
|------|--------------------|------------|
| `ThrowableNotSerializableException` 相当 | serializer できなかった例外の message と class name を payload として保持する | message / class name の保持、clone / equality |
| `FailureDetectorWithAddress` | detector に監視先 address を後から設定し、参照できる | 初期状態は address なし、設定後に address を返す |
| `DeadlineFailureDetector` | heartbeat の deadline を超えるまで available、超えたら unavailable | 未監視状態、境界直前、境界値、境界超過 |
| `RemoteLogMarker` | Pekko と同じ marker 名と remote address property を `ActorLogMarker` として返す | marker 名、address property、uid property |

## 実装ステップ

1. 既存の serialization / failure detector / instrument テスト配置を確認する。
2. 既存パターンに合わせて `{type}/tests.rs` へ単体テストを追加する。
3. 3 モジュール以上を横断する新しいデータフローはないため、インテグレーションテストは追加しない。
4. 実装前の失敗を確認するため、変更範囲の単体テストを実行する。

## スコープ外

`advertised addresses / listen event`、`lifecycle event publishing from association effects`、remote actor ref 生成、payload serialization、Artery framing、DeathWatch 統合は今回のテスト対象外とする。
