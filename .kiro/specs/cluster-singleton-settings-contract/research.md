# 調査ログ: cluster-singleton-settings-contract

## ギャップ分析（kiro-validate-gap, 2026-06-11）

### 1. 現状調査

#### 先行例: FailureDetectorConfig パターン（configure-cluster-failure-detector で確立）

| 構成要素 | 現状の実装 | 場所 |
|----------|-----------|------|
| 設定型 | `FailureDetectorConfig`（`Copy` 可能 struct、`with_*` setter chain、`Default` は `new()` 委譲） | `cluster-core-kernel/src/failure_detector/failure_detector_config.rs` |
| 検証 | `validate(&self) -> Result<(), FailureDetectorConfigError>` を自型が所有。`ClusterExtensionConfig::validate()` が委譲呼び出し | 同上 + `extension/cluster_extension_config.rs` |
| install 境界 | `ClusterExtensionInstaller::install` が `config.validate()` を実行し `ActorSystemBuildError::Configuration` で拒否 | `extension/cluster_extension_installer.rs` |
| 互換キー | `ClusterCompatibilityKeyCatalog::FAILURE_DETECTOR`（`required("cluster.failure-detector")`）。`ClusterExtensionConfig` が `JoinConfigCompatChecker` を実装し、`JOIN_COMPATIBILITY_CHECKS` 配列の `JoinCompatibilityCheck { key, mismatch_detail }` で差異フィールド名を列挙 | `topology/cluster_compatibility_key_catalog.rs` 等 |
| 不一致理由 | `ConfigValidation::Incompatible { reason: String }`。`JoinCompatibilityComposition` が複数 checker の理由を `;` で結合 | `topology/config_validation.rs` 等 |
| std 接続 | `ConfiguredPhiAccrualDetectorFactory`（設定 → 実装の factory） | `cluster-adaptor-std/src/membership/` |

#### setup 統合点の現状

- 構築フロー: `BootstrapSetup` → `ActorSystemSetup::with_extension_installers(ExtensionInstallers)` → `ActorSystemConfig`
- `ExtensionInstaller` trait（`fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError>`）を `ClusterExtensionInstaller` が実装
- **typed 層の前例**: `cluster-core-typed/src/cluster_setup.rs` の `ClusterSetup` が `ExtensionInstaller` を実装する薄い setup ラッパー。「ClusterSingletonSetup 相当」はこの前例に直接対応する
- Pekko typed の `ClusterSingletonSetup` は Extension 実装差し替え用のテストフックであり、設定値保持の責務はない（fraktor で再現必須ではない）

#### 観測契約の現状

- 慣行: `ClusterEvent` enum に variant 追加 → `EventStreamEvent::Extension { name: "cluster", payload }` で publish。購読は `ClusterApi::subscribe` + `ClusterEventType` フィルタ
- 参考 variant: `UnreachableMember` / `TopologyApplyFailed`（検知内容 + `observed_at: TimerInstant`）
- cluster-membership-event-surface で「専用イベント併発 + ClusterEventType フィルタ + 購読統合テスト」のパターンが直近に確立済み

#### lease 語彙の現状

- 汎用の lease 設定型（Pekko `LeaseUsageSettings` 相当）は**存在しない**
- 部分的な語彙: `LeaseAcquisitionOutcome`（kernel/downing_provider）、`lease_majority_backend`（adaptor-std、SBR 用）。SBR 文脈に限定されており、singleton から再利用できる汎用設定型ではない

#### 命名の precedent

- fraktor 独自ドメイン: `*Config`（`FailureDetectorConfig` / `ClusterExtensionConfig` / `MembershipCoordinatorConfig` / `PubSubConfig`）
- Pekko 逆輸入名: `*Settings`（`DistributedPubSubSettings` / `SplitBrainResolverSettings`）
- → singleton 設定は Pekko 逆輸入名のため `ClusterSingleton*Settings` に precedent がある（最終決定は設計フェーズ）

#### typed 層の構造

- `cluster-core-typed` はフラットなファイル構成（`cluster.rs` / `cluster_setup.rs` / `cluster_command.rs` 等）。typed 統合設定はここに新規ファイルで置くのが自然

### 2. 要件と既存資産のマッピング

| 要件 | 既存資産 | ギャップ | タグ |
|------|----------|----------|------|
| 1. Manager 設定契約 | `FailureDetectorConfig` パターン（setter chain / Default / validate） | singleton manager 設定型そのものが無い。removal margin の「未指定」表現（`Option<Duration>` 相当）が必要 | Missing |
| 2. Proxy 設定契約 | 同上 + `DataCenter` 型（membership） | proxy 設定型が無い。buffer size 範囲検証（0〜10000）の前例はある（数値範囲検証） | Missing |
| 3. typed 統合設定 | `cluster-core-typed` のフラット構成、`ClusterIdentity` 等の typed 型 | typed 統合設定型と manager / proxy への導出 API が無い。kernel ↔ typed の依存方向は既存（typed → kernel）どおり | Missing |
| 4. 設定検証 | `ClusterExtensionConfig::validate()` の委譲チェーン、`FailureDetectorConfigError` の形 | singleton 用エラー型と委譲呼び出しの追加が必要。パターンは確立済み | Missing（パターンは Constraint: 既存形式に従う） |
| 5. Join Compatibility | `JOIN_COMPATIBILITY_CHECKS` 配列、`ClusterCompatibilityKeyCatalog`、`difference_field_names` 方式 | singleton 用キー（例: `cluster.singleton`）と mismatch_detail 関数の追加が必要。配列拡張のみで合成側は無変更 | Missing（接続点は既存） |
| 6. setup 統合点 | `ClusterExtensionConfig` へのフィールド追加 + `ClusterSetup`（typed）前例 | singleton 設定を `ClusterExtensionConfig` に載せれば install 境界は既存のまま成立。未指定時の既定値適用は `Default` で充足 | Missing（最小差分） |
| 7. Stuck 観測契約 | `ClusterEvent` variant + `ClusterEventType` フィルタの確立済みパターン、`TimerInstant` | stuck 検知条件（リトライ上限導出）の定義と通知型が無い。**検知を実行する runtime は本 spec 対象外**のため「条件の決定的導出関数 + 通知 variant」という純粋契約に留める必要がある | Missing + Unknown（通知の載せ方に選択肢） |
| 8. 非回帰・範囲限定 | 既存テスト群（cluster 3 クレート）、`ClusterEventType::matches` の網羅 match | 新規追加のみで既存変更を避ける設計が必要（cluster-membership-event-surface と同じ制約） | Constraint |

### 3. 実装アプローチの選択肢

#### Option A: ClusterExtensionConfig 拡張一本化（既存コンポーネント拡張）

singleton 設定 3 型を新規ファイルで定義しつつ、検証・互換キー・install はすべて `ClusterExtensionConfig` への組み込みで賄う。

- 変更: `ClusterExtensionConfig` にフィールド + validate 委譲 + `JOIN_COMPATIBILITY_CHECKS` エントリ追加。stuck 通知は `ClusterEvent` variant 追加
- ✅ FailureDetectorConfig と完全に同型。要件 5（互換キー連動）・要件 6（既定値 install）が最小差分で成立
- ✅ 「設定だけ存在して配線されない」状態を構造的に回避できる（brief の制約）
- ❌ `ClusterExtensionConfig` が肥大化していく（設定ドメインが増えるたびにフィールドが増える構造的傾向）

#### Option B: 独立 installer / 独立 extension 化（新規コンポーネント）

`ClusterSingletonExtensionInstaller` を別 installer として新設し、設定・検証を cluster 本体から分離する。

- ✅ singleton ドメインの分離が明確。Phase 3 の runtime extension にそのまま接続できる
- ❌ Join Compatibility は `ClusterExtensionConfig` の checker 配列に依存しているため、独立 extension にすると互換キー連動（要件 5）に別経路が必要になり、要件に対して過剰な構造
- ❌ 「既存 cluster 設定と同じ入口」（要件 6）から外れる

#### Option C: ハイブリッド（推奨候補）

設定型・エラー型・stuck 契約は独立した新規ファイル群（singleton 専用モジュール）として定義し、配線（validate 委譲・互換キー・install）は `ClusterExtensionConfig` 拡張で行う。typed 統合設定は `cluster-core-typed` に新規ファイルで置き、kernel の manager / proxy 設定へ導出する。

- ✅ 型の所有は新規モジュールに分離（1 ファイル 1 公開型、テスト 1:1 を維持）しつつ、配線は実績ある経路を再利用
- ✅ FailureDetectorConfig（failure_detector/ サブモジュール所有 + extension/ で配線）と同じ構造であり、実質的に確立済みパターンの適用
- ❌ A との差は「設定型を extension/ 直下に置くか専用サブモジュールに置くか」程度で、選択肢としての距離は小さい

### 4. 工数とリスク

| 項目 | 評価 | 根拠 |
|------|------|------|
| 工数 | **M（3〜7 日相当）** | 新規公開型 6〜8 個（manager / proxy / typed 統合 / エラー型 ×2〜3 / stuck 通知）+ 配線 + テスト。パターンは全て確立済みで未知の統合は無い |
| リスク | **Low** | FailureDetectorConfig / cluster-membership-event-surface の 2 つの直近 precedent をなぞる。アーキテクチャ変更なし、既存変更は配線数行のみ |

### 5. 設計フェーズへの推奨と Research Needed

**推奨**: Option C（型は singleton 専用モジュール所有、配線は ClusterExtensionConfig / ClusterEvent の既存経路）。

**Research Needed（設計フェーズで確定）**:

1. **命名（`*Settings` vs `*Config`）**: Pekko 逆輸入名の precedent（`SplitBrainResolverSettings` 等）に従い `ClusterSingletonManagerSettings` / `ClusterSingletonProxySettings` / `ClusterSingletonSettings` とするか。fraktor の `*Config` 主流との二段慣行を design で明文化する
2. **stuck 通知の載せ方**: `ClusterEvent` variant 追加（購読フィルタ連動、cluster-membership-event-surface と同型）か、通知型のみ定義して発行点を Phase 3 に委ねるか。要件 7.3（他イベントと区別して識別）は `ClusterEventType` フィルタで満たすのが自然だが、発行 runtime が無い段階で variant を追加すると「発行されない variant」が生まれる。発行点の真空性をどう扱うかを design で決める（trace field 契約の先例: 消費者を同 spec 内で用意して真空を回避した）
3. **リトライ上限の導出式**: Pekko は `max(minRetries, removalMargin / handOverRetryInterval + 3)`。minRetries 相当を設定項目に含めるか固定値にするか
4. **removal margin 未指定の表現**: `Option<Duration>` か、ゼロ値をセンチネルにするか（要件 1.3 は「明示済みの値と区別できる形」を要求するため `Option` 相当が素直）
5. **lease スロットの型配置**: SBR の lease 語彙（`LeaseAcquisitionOutcome`）とは独立した設定専用型（実装名 + リトライ間隔のみ）として定義するか、将来の共通 lease 契約を見据えた配置にするか
6. **singleton 設定の data center 項目と `DataCenter` 型の再利用**: proxy 設定の data center 項目に membership の `DataCenter` を使うか（依存方向: extension → membership は既存で許容済みかを design で確認）

---

## 設計ディスカバリー（kiro-spec-design, 2026-06-11）

### 追加調査結果

- **Pekko 既定値**（`cluster-tools/reference.conf`）: `hand-over-retry-interval = 1s`、`min-number-of-hand-over-retries = 15`、`singleton-identification-interval = 1s`、`buffer-size = 1000`（proxy）。リトライ上限はコメントで「hand-over-retry-interval と removal margin から導出」と明記
- **kernel モジュール構成**: `lib.rs` のトップレベルは `activation / cluster_provider / downing_provider / extension / failure_detector / grain / membership / message_serialization / outbound / pub_sub / topology`。singleton は `failure_detector` と同格の新規トップレベルモジュールが自然
- **ClusterExtensionConfig の現状**: 既に downing_provider / failure_detector / pub_sub の 3 ドメインを集約し、互換キーは「failure detector = 設定全体で 1 キー + 差異フィールド列挙」「pubsub = フィールド単位キー」の 2 流儀が併存。singleton は failure detector 方式（1 キー + `difference_field_names`）が要件 5.2（項目特定可能な理由）に適合
- **topology → 他モジュール参照の前例**: `cluster_event.rs` は membership の型（`DataCenter` / `NodeStatus` 等)を payload として import 済み。singleton 語彙（stuck 局面 enum）を ClusterEvent が参照するのは同型

### Research Needed の決定

| # | 論点 | 決定 | 根拠 |
|---|------|------|------|
| 1 | 命名 | `ClusterSingletonManagerSettings` / `ClusterSingletonProxySettings` / typed `ClusterSingletonSettings` / `LeaseUsageSettings` | Pekko 逆輸入名は `*Settings`（`SplitBrainResolverSettings` / `DistributedPubSubSettings` の repo 内 precedent）。reference-implementation 命名規約に従う |
| 2 | stuck 通知の載せ方 | `ClusterEvent::SingletonHandOverStuck` variant + `ClusterEventType` variant を本 spec で追加。発行 runtime は Phase 3 | 要件 7.3（他イベントと区別して識別）は `ClusterEventType` フィルタでのみ自然に満たせる。識別契約は EventStream 経由でテスト発行すれば runtime なしで検証可能（真空ではなくテストが消費者になる）。発行点の追加は再検証トリガーに記録 |
| 3 | リトライ上限導出式 | `max_hand_over_retries() = max(min_hand_over_retries, removal_margin / hand_over_retry_interval + 3)`。`min_hand_over_retries: u32` を設定項目に含める（既定 15） | Pekko の導出式・既定値と一致。設定の純粋メソッドとして定義すれば runtime なしで決定性をテスト可能（要件 7.1） |
| 4 | removal margin 未指定 | `Option<Duration>`（`None` = downing 側の margin に従う）。導出式では `None` を 0 として扱い、Phase 3 が解決済み margin で設定を再構成する | 要件 1.3 の「明示済みの値と区別できる形」にセンチネルより型安全に適合 |
| 5 | lease スロット | `LeaseUsageSettings { lease_implementation: String, lease_retry_interval: Duration }` を singleton モジュール内の独立型として定義。SBR の lease 語彙（`LeaseAcquisitionOutcome`）とは結合しない | 設定契約に必要な 2 項目のみ。将来の共通 lease 契約はインターフェースが安定してから（YAGNI） |
| 6 | DataCenter 再利用 | proxy 設定の data center 項目は `membership::DataCenter` の `Option` を再利用 | cluster_event.rs の前例どおり kernel 内モジュール間参照は許容。新規の文字列型を作らない |

### 統合方針の確定（synthesis）

- **一般化**: typed `ClusterSingletonSettings` を manager / proxy 双方の射影元として設計（`to_manager_settings(name)` / `to_proxy_settings(name)`、Pekko typed と同構造）。インターフェースの一般化のみで実装の先取りはしない
- **Build vs Adopt**: 外部依存なし。FailureDetectorConfig（設定 + validate + 互換キー）、ClusterSetup（typed setup）、ClusterEvent（観測語彙）の repo 内パターンをそのまま採用
- **単純化（落としたもの）**:
  - Pekko `ClusterSingletonSetup`（Extension 差し替え用テストフック）は再現しない。要件 6 は `ClusterExtensionConfig` への設定統合 + 既定値 `Default` で成立し、専用 Setup 型は不要
  - stuck 検知の状態機械（リトライ計数）は定義しない。要件 7 は「導出式 + 通知語彙 + 識別」の純粋契約で閉じ、計数は Phase 3 runtime の責務
  - std 層の追加なし（factory が必要になるのは runtime 実装時）
