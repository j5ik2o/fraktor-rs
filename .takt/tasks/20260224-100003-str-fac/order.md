# タスク仕様

## 目的

streamsモジュールに不足しているファクトリメソッドとtrivial/easyオペレーター（Phase 1-2）を実装し、Pekko Streamsとの基本互換性を強化する。

## 要件

- [ ] `Sink::contramap` オペレーターを実装する（入力型の事前変換）
- [ ] `ThrottleMode` enum（`Shaping` / `Enforcing`）を実装する（throttleの挙動制御）
- [ ] `distinct` / `distinctBy` オペレーターを実装する（重複排除）
- [ ] `Source::from_graph` / `Flow::from_graph` / `Sink::from_graph` ファクトリを実装する
- [ ] `Source::pre_materialize` を実装する（マテリアライゼーション前のActorRef取得）
- [ ] `KillSwitches::single_bidi` を実装する（双方向KillSwitch）
- [ ] 各機能に対するテストを追加する

## 受け入れ基準

- 各オペレーター/ファクトリが既存のStreams DSLと一貫した形で提供される
- ThrottleModeがthrottleオペレーターと統合されている
- `./scripts/ci-check.sh all` がパスする

## 参考情報

- ギャップ分析: `docs/gap-analysis/streams-gap-analysis.md`
- Pekko参照: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/`
