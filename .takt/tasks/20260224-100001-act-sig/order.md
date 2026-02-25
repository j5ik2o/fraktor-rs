# タスク仕様

## 目的

actorモジュールに不足しているシグナル型とウォッチ拡張（Phase 1-2: trivial〜easy）を実装し、Pekko Typed Actorとの互換性を向上させる。

## 要件

- [ ] `MessageAdaptionFailure` シグナルを `BehaviorSignal` enumに新バリアント追加として実装する
- [ ] `watchWith(target, msg)` を実装する（監視対象の終了時にカスタムメッセージを送信）
- [ ] `ChildFailed` シグナルを実装する（子アクター失敗時のシグナル通知）
- [ ] `PreRestart` シグナルを実装する（リスタート前のライフサイクルフック）
- [ ] `DeathPactException` を実装する（未ハンドルの `Terminated` 時のデフォルト動作定義）
- [ ] 既存テストが破壊されないことを確認し、新機能に対するテストを追加する

## 受け入れ基準

- すべての新シグナル型が `BehaviorSignal` または関連enumに統合されている
- `watchWith` がTypedActorContextから利用可能
- `./scripts/ci-check.sh all` がパスする
- 各機能に対するユニットテストが存在する

## 参考情報

- ギャップ分析: `docs/gap-analysis/actor-gap-analysis.md`（カテゴリ4: シグナル拡張、カテゴリ5: ウォッチ拡張）
- Pekko参照: `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Signal.scala`
- Pekko参照: `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/ActorContext.scala`
