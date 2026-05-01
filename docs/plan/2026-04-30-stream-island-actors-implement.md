# stream-island-actors 実装計画

## 対象

`stream-island-actors` change の今回バッチ 2.5 / 3.1〜3.3 / 4.1〜4.4 を実装する。

## 実装方針

- `ActorMaterializer::start()` から単一 `StreamDriveActor` と materializer-wide tick を削除し、起動時は lifecycle state だけを `Running` にする。
- `ActorMaterializer::materialize(...)` で `IslandSplitter::split(...)` の island ごとに `StreamShared` と `StreamIslandActor` を作成する。
- `SingleIslandPlan::dispatcher()` を `into_stream_plan()` 前に読み、指定がある場合だけ `Props::with_dispatcher_id(...)` を適用する。
- 各 island actor に専用 scheduler job を登録し、scheduler callback は `StreamIslandCommand::Drive` の送信だけを行う。
- materialized graph ごとの streams / island actors / scheduler handles を内部構造で追跡する。
- `ActorMaterializer::shutdown()` で全 scheduler handle を cancel し、全 island actor へ `StreamIslandCommand::Shutdown` を送る。
- `StreamDriveActor` / `StreamDriveCommand` と module wiring を削除し、旧 drive 経路の参照を残さない。
- 実装完了後に `openspec/changes/stream-island-actors/tasks.md` の対象チェックを更新する。

## 検証

- `rtk rustup run nightly-2025-12-01 cargo fmt --all --check`
- `rtk cargo test -p fraktor-stream-core-rs actor_materializer stream_island`
- `rtk grep "StreamDriveActor|StreamDriveCommand" modules/stream-core/src`
- `rtk git diff --check`
