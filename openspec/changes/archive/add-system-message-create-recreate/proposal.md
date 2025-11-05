# 提案: Create/Recreate SystemMessage による起動・再起動の統一

## Why

- 現状 `ActorCell::create`/`restart` が `pre_start` / `post_stop` を直接呼び出し、メールボックス経由の制御と乖離している。
- Protoactor/Pekko では起動・再起動も `SystemMessage` として流れ、監督処理との整合性を保っている。
- DeathWatch 実装により `SystemMessage` が制御面の入口になりつつあるため、Create/Recreate も揃えることで一貫性を高めたい。

## What Changes

1. `SystemMessage` に `Create` / `Recreate` variant を追加し、ActorCell が受信時に `pre_start`/`post_stop` を処理する。
2. spawn / restart フローは `SystemMessage::Create/Recreate` を enqueue するだけにし、ActorCell 側で完結させる。
3. Supervisor / lifecycle 通知のテストを更新し、`post_stop` → `pre_start` (Restart) の順序が SystemMessage 経由で保証されることを確認する。

## Impact

- 制御メッセージが mailbox で一本化され、将来的な Failure SystemMessage 導入への足掛かりになる。
- 既存 API には破壊的変更なし。ただし内部の起動順序が変わるためテスト増強が必要。
