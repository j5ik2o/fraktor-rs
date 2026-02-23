# タスク仕様

## 目的

remoteモジュールに障害検出（FailureDetector）の型階層とレジストリ（Phase 1-3: trivial〜medium）を実装し、Pekkoの障害検出基盤と同等の抽象化を提供する。

## 要件

- [ ] `FailureDetector` traitを実装する（`is_available`, `is_monitoring`, `heartbeat` の3メソッド）
- [ ] `FailureDetectorWithAddress` traitを実装する（`set_address` メソッド追加）
- [ ] 既存の `PhiFailureDetector` を `FailureDetector` traitの実装とする
- [ ] `DeadlineFailureDetector` を実装する（deadlineベースの単純な障害検出）
- [ ] `FailureDetectorRegistry<A>` traitを実装する（リソース別FD管理: `is_available`, `heartbeat`, `remove`, `reset`）
- [ ] `DefaultFailureDetectorRegistry` を実装する（Registryの標準実装）
- [ ] 各機能に対するテストを追加する

## 受け入れ基準

- FailureDetector traitが抽象的なインターフェースとして定義され、PhiFailureDetectorが実装している
- DeadlineFailureDetectorが独立した障害検出器として動作する
- FailureDetectorRegistryでリソース（ノード）単位のFD管理が可能
- `./scripts/ci-check.sh all` がパスする

## 参考情報

- ギャップ分析: `docs/gap-analysis/remote-gap-analysis.md`（カテゴリ2: 障害検出）
- Pekko参照: `references/pekko/remote/src/main/scala/org/apache/pekko/remote/FailureDetector.scala`
- Pekko参照: `references/pekko/remote/src/main/scala/org/apache/pekko/remote/PhiAccrualFailureDetector.scala`
- Pekko参照: `references/pekko/remote/src/main/scala/org/apache/pekko/remote/DeadlineFailureDetector.scala`
- Pekko参照: `references/pekko/remote/src/main/scala/org/apache/pekko/remote/FailureDetectorRegistry.scala`
