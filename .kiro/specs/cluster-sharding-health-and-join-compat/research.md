# 調査・設計判断

## 要約

- **機能**: `cluster-sharding-health-and-join-compat`
- **ディスカバリー範囲**: 拡張（既存 join 互換基盤と grain runtime 状態への接続、light discovery + 参照実装比較）
- **主要な発見**:
  - join 互換チェックは `ClusterExtensionConfig` 同士の in-process 比較であり、grain / placement 関連の設定は現在 `ClusterExtensionConfig` に一切載っていない。`PartitionIdentityLookupConfig`（cache_capacity / pid_ttl_secs / idle_ttl_secs）は installer の factory 閉包内で使われる設定で config 非所有、かつ全 field がローカルチューニング値
  - identity lookup の実装選択（Noop / Partition / 利用者定義）は `ClusterExtensionInstaller::with_identity_lookup_factory` の閉包注入であり、`cluster.failure-detector.choice` が excluded key にされたのと同一の「config が選択を所有していない」状況
  - したがって brief が想定した「partition 数、identity lookup 設定などノード間で一致すべき値」は現行設定に存在しない。required な join 比較は非自明に成立せず、除外キーの目録整備が現状の正しい貢献（人間の確認で承認済み: 除外キーのみで整備、required 比較は Phase 2 の包括設定契約へ委譲）
  - readiness 判定に必要な入力はすべて core の `&self` クエリで観測可能: 自ノード membership 状態（`CurrentClusterState.members` の `NodeStatus`）、placement 調整状態（`PlacementCoordinatorState`: Stopped / Member / Client / NotReady）、kind 登録状態（`KindRegistry::contains` / `all`）

## 調査ログ

### join 互換チェック基盤の現状

- **背景**: 互換キー追加の接続点と先行例の特定
- **参照した情報源**: `modules/cluster-core-kernel/src/topology/cluster_compatibility_key_catalog.rs`, `topology/cluster_compatibility_key.rs`, `topology/join_compatibility_composition.rs`, `extension/cluster_extension_config.rs`, `membership/membership_coordinator.rs`
- **発見**:
  - `ClusterCompatibilityKeyCatalog` は `REQUIRED_KEYS`（pubsub / downing-provider / failure-detector / singleton）、`CONDITIONAL_KEYS`（SBR config）、`EXCLUDED_KEYS`（advertised-address / downing-provider.factory / failure-detector.choice）の3配列と `required_keys()` / `conditional_keys()` / `excluded_keys()` アクセサを持つ
  - required 比較の先行例（singleton / failure detector）は「単一キー + `difference_field_names()` による差分 field 名 detail + `append_mismatch_reason` での `"<key> mismatch: <fields>"` 整形」パターン
  - 評価は `MembershipCoordinator::handle_join` → `ClusterExtensionConfig::check_join_compatibility(joining)` の in-process 比較。不一致は `MembershipError::IncompatibleConfig { reason }` で join 拒否
  - excluded key の理由文の先行例: `"sensitive local factory implementation is not compared during join compatibility"`（downing-provider.factory）、`"failure detector implementation choice is not compared until cluster config owns detector selection"`（failure-detector.choice）
- **含意**: grain / placement 領域の貢献は EXCLUDED_KEYS への2キー追加（identity lookup の実装選択、ローカルチューニング値）と目録テストの更新で完結する。合成評価のロジックは無変更

### grain runtime の readiness 入力

- **背景**: readiness 判定の入力が core の観測可能なクエリで揃うかの確認
- **参照した情報源**: `grain/kind_registry.rs`, `activation/placement_coordinator.rs`, `membership/node_status.rs`, `membership/current_cluster_state.rs`, `extension/cluster_extension.rs`
- **発見**:
  - `KindRegistry::contains(name) -> bool` / `all() -> Vec<ActivatedKind>`（`&self`）
  - `PlacementCoordinatorCore::state() -> PlacementCoordinatorState`。`NotReady` / `Stopped` では resolve が `LookupError::NotReady` を返す（= 解決不能）。`Member` / `Client` が解決可能状態
  - `NodeStatus` は Joining / WeaklyUp / Up / Suspect / ... / Dead。Pekko の membership health check は Up / WeaklyUp を受け入れ状態として扱う
  - `ClusterExtension` には `virtual_actor_count()` のような core への薄い読み取りアクセサの先行例がある
- **含意**: 判定は「入力スナップショット（値オブジェクト）+ pure なクエリ」として core に置ける。スナップショット構築は core の既存状態からの読み取りで完結し、std adapter は判定を呼ぶ薄い関数になる

### Pekko 参照実装

- **参照した情報源**: `references/pekko/cluster-sharding/src/main/scala/org/apache/pekko/cluster/sharding/ClusterShardingHealthCheck.scala`, `JoinConfigCompatCheckSharding.scala`
- **発見**:
  - `ClusterShardingHealthCheck`: 設定された region 名集合の全 region が coordinator 登録済みなら true。一度 true になると以後 true を返し続ける（sticky）。timeout は false
  - `JoinConfigCompatCheckSharding`: 比較キーは `state-store-mode` の1つだけ（full string match）。Pekko は config がこの選択を所有しているから成立する
- **含意**: fraktor の readiness は「region 登録」の代わりに「membership 自ノード状態 + placement 解決可能性 + 期待 kind 登録」を入力とする。sticky 挙動は状態を持つため pure 判定とは分離し、本 spec では採用しない（必要なら呼び出し側の責務）。state-store-mode 相当の選択は fraktor では config 非所有のため required 比較は Phase 2 へ

## アーキテクチャパターン評価

| 選択肢 | 説明 | 強み | リスク／制約 | メモ |
|--------|-------------|-----------|---------------------|-------|
| スナップショット + pure クエリ（採用） | 入力値オブジェクトに判定クエリを持たせ、構築は extension の読み取りアクセサで行う | 判定が決定的・テスト容易、no_std 完結（要件 1.7, 4.3） | 入力の鮮度は呼び出し時点に固定 | readiness probe の用途では十分 |
| 判定を std adapter に直書き | adapter が状態を読みながら判定 | 型が減る | 判定規則が host 層に漏れ、決定性・単体テスト性を失う（要件 2.2 違反） | 却下 |
| std の便宜ラッパー関数 | core のアクセサ + 判定を呼ぶだけの公開関数を adaptor-std に置く | 発見しやすい std API | core を呼ぶだけの層は「core が契約を定義しホスト層が従う」依存方向原則に反する（dylint で強制） | 人間の指摘で却下（公開手段は core のアクセサ自体とする） |
| sticky な health check 状態機械 | Pekko 同様、一度 ready で固定 | LB フラッピング耐性 | 状態を持ち pure 判定でなくなる。要件 1.1/1.7 と衝突 | 却下（呼び出し側で実現可能） |
| config 所有化 + required key | identity lookup 選択を config 所有にし required 比較 | 不一致 join を実際に拒否できる | installer 配線変更・factory との drift リスク・スコープ拡大 | 人間の判断で却下（Phase 2 へ委譲） |

## 設計判断

### 判断: readiness は `GrainReadinessSnapshot`（入力値）+ `readiness(expected_kinds)` クエリとして core/grain に定義する

- **背景**: 要件 1.1–1.7（観測可能な入力のみからの純粋導出、理由の識別）
- **検討した代替案**: (1) evaluator 型の新設 — 入力と規則が1対1なのでスナップショットのクエリで足りる（YAGNI）。(2) `bool` を返す — 理由（要件 1.3–1.5）を運用者へ返せない
- **採用したアプローチ**: スナップショット（自ノード status / placement 状態 / 登録済み kind）に対する pure クエリが `GrainReadiness`（Ready / NotReady{reasons}）を返す。reasons は `GrainUnreadyReason`（自ノード非稼働 / placement 解決不能 / kind 未登録）で原因種別を固定
- **トレードオフ**: 公開型は3つ（Pekko の health check 1 型 + settings 1 型に対し 1.5 倍以内）

### 判断: 稼働状態は `Up | WeaklyUp`、解決可能状態は `Member | Client` とする

- **背景**: 要件 1.2 の「稼働状態」「解決可能な状態」の具体化
- **根拠**: Pekko の membership health check が Up / WeaklyUp を受け入れ状態とする先行例。`PlacementCoordinatorState` は `NotReady` / `Stopped` で resolve が失敗する実装事実
- **トレードオフ**: WeaklyUp を ready に含めるため、unreachable ノードがいる間に join したノードもトラフィックを受けうる（Pekko と同じ割り切り）。rustdoc に固定仕様として明記

### 判断: 公開手段は `ClusterExtension` の読み取りアクセサ自体とし、std 側に便宜層を作らない

- **背景**: 要件 2.1–2.3 と「core が port / 契約を定義し、ホスト層がそれに従う。逆方向は dylint で強制的に禁止」という設計原則（人間の指示で確定）
- **検討した代替案**: adaptor-std に `grain_readiness(extension, kinds)` の公開関数を置く — core API を呼ぶだけの便宜層であり、ホスト機能を一切使わないため依存方向原則に反する。却下
- **採用したアプローチ**: `ClusterExtension::grain_readiness_snapshot()`（`&self`、core 状態の読み取りのみ。`virtual_actor_count()` の先行パターン踏襲）を公開手段とする。core API は std を含むホスト環境からそのまま呼び出せる。brief の「std adapter（関数形態）」想定はこの原則を優先して上書きする
- **トレードオフ**: 既存ファイル（extension / core）への追加変更が入るが、読み取り専用メソッドの追加であり挙動不変（要件 4.1）

### 判断: placement 状態の観測は `IdentityLookup` port のデフォルト実装付きクエリとして追加する

- **背景**: sanity review の指摘 — `ClusterCore` が保持するのは `dyn IdentityLookup` であり、trait は `PlacementCoordinatorState` を公開していない（`PartitionIdentityLookup` 内部の `PlacementCoordinatorCore::state()` に隠れている）。アクセス経路が未確立のままでは実装タスクの scope が暴れる
- **検討した代替案**: (1) `PlacementCoordinatorShared` を extension に直接持たせる — lookup 実装の内部構造への依存で port 境界を壊す。(2) `StartupMode` から代用導出 — placement 調整の実状態（NotReady 等）を反映できない
- **採用したアプローチ**: trait に `fn placement_state(&self) -> PlacementCoordinatorState { PlacementCoordinatorState::NotReady }` を追加。デフォルトが `NotReady` を返すため既存実装型（`NoopIdentityLookup`、テストローカル実装）は無変更（trait の `resolve` デフォルトが `NotReady` を返す先行例と整合）。`PartitionIdentityLookup` だけが override する
- **トレードオフ**: port の操作が1つ増えるが、core が定義する読み取りクエリ（`&self`、CQS 準拠）であり依存方向原則そのもの

### 判断: 自ノードの membership 状態は既存 `current_cluster_state_snapshot()` の自ノード record から取得する

- **背景**: sanity review の指摘 — 既存実装は在籍メンバーの status を `Up` でハードコードしており、Joining / WeaklyUp 等の実状態を追跡する公開経路がない
- **検討した代替案**: core に自ノード status の実追跡を新設する — membership 状態遷移の変更に踏み込み、要件 4.1（挙動不変）とスコープ（忠実度向上は別件）を超える
- **採用したアプローチ**: 既存スナップショットの自ノード record をそのまま使う。現状の忠実度では「在籍 = Up、不在 = None」の判定になることを design / rustdoc に明記する。判定は入力駆動なので、core の状態忠実度が将来向上すれば readiness はそのまま追従する
- **トレードオフ**: 現時点の「稼働状態」は実質「topology 上の在籍」を意味する（正直に文書化して受容）

### 判断: join 互換キーは EXCLUDED_KEYS への2キー追加に限定する

- **背景**: 要件 3.1–3.3。調査の結果、required 比較が成立する現行設定が存在しない
- **採用したアプローチ**: `cluster.sharding.identity-lookup.choice`（factory 注入で config 非所有のため比較しない）と `cluster.sharding.identity-lookup.tuning`（ローカルチューニング値のため一致不要）を除外理由付きで登録。理由文は既存 excluded key の文体を踏襲
- **根拠**: `cluster.failure-detector.choice` の先行例と同型。合成評価ロジック・既存キーの評価結果は不変（要件 3.2, 4.2）
- **フォローアップ**: Phase 2 の包括設定契約が config 所有化とともに required key を追加する（要件 3.3 の選定基準を引き継ぐ）

## リスクと緩和策

- **readiness の判定規則が暗黙に変わるリスク** — 稼働状態・解決可能状態の集合を rustdoc に固定仕様として明記し、sibling テストで全分岐（status × placement × kind の代表組み合わせ）を固定
- **スナップショットの鮮度誤解** — rustdoc に「呼び出し時点の状態の写しであり、継続監視は呼び出し側の責務」と明記
- **除外キー名の将来衝突** — Phase 2 で required key を追加する際は別名（例: `cluster.sharding`)を使い、除外キーはそのまま残す方針を目録テストに記録

## 参考資料

- `references/pekko/cluster-sharding/.../ClusterShardingHealthCheck.scala` — readiness 判定の参照実装
- `references/pekko/cluster-sharding/.../JoinConfigCompatCheckSharding.scala` — join 互換キーの参照実装（state-store-mode 単一キー）
- `.kiro/specs/cluster-sharding-health-and-join-compat/brief.md` — discovery 決定事項
- `docs/gap-analysis/cluster-gap-analysis.md` — カテゴリ8（health check）/ カテゴリ10（join compat）
