1. [ ] `modules/actor-std/src/typed` パッケージ構造を追加し、core 版と同等のサブモジュール（actor_prim/behaviors/props/system 等）を用意する
2. [ ] std 用 typed トレイト (`TypedActor` など) と adapter を `actor_prim` 配下に実装し、既存 `Actor` トレイトと橋渡しする
3. [ ] `TypedProps`, `TypedActorRef`, `TypedActorSystem` などのラッパーを std 向けに実装し、`StdToolbox` へ接続する
4. [ ] example と docs を追加/更新し、std typed API の利用方法（Behaviors::setup/receiveMessage/receiveSignal 等）を示す
5. [ ] 単体テストと `./scripts/ci-check.sh all` を実行し、std typed 実装が既存 CI を通過することを確認する
