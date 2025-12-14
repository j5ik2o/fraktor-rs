# ギャップ分析: system-state-generic-option-removal

> 注意: 本 spec は requirements が生成済みだが `approved` ではない（spec.json）。ただし実装方針検討の材料としてギャップ分析は先行して作成する。

## 1. 現状把握
- **SystemStateGeneric の未初期化フィールド**: `modules/actor/src/core/system/system_state.rs` にて `remote_watch_hook/scheduler_context/tick_driver_runtime/remoting_config` を `ToolboxMutex<Option<...>>` で保持し、`SystemStateGeneric::new()` ではすべて `None` で生成される。
- **後差し込み API**:
  - `apply_actor_system_config()` が `PathIdentity` と `remoting_config` を更新する（構築後に構成適用）。
  - `install_scheduler_context()` / `install_tick_driver_runtime()` が `Option` を `Some` へ置き換える（構築後に依存物を注入）。
  - `register_remote_watch_hook()` が remote watch hook を `Some` へ置き換える。
- **ActorSystemGeneric の生成フロー**: `modules/actor/src/core/system/base.rs` で `new_with_config_and()` が内部で `new_empty()` を呼び、その後に `apply_actor_system_config()` と `install_scheduler_and_tick_driver_from_config()` を実行している。つまり「SystemStateGeneric を作ってから初期化する」順序が現状の常道になっている。
- **new_empty() の位置付け**: `ActorSystemGeneric::new_empty()` は現状 `cfg(test)` でも `feature = "test-support"` でもなく公開されており、さらにプロダクション生成経路（`new_with_config_and`）の内部実装でも使用されている。テストでは多数のユニットテストが `new_empty()` を前提にしている。
- **TickDriver / Scheduler の前提**:
  - `SchedulerContextSharedGeneric` は共有ラッパ（内部 `RwLock`）として成立しているが、SystemState 側では `Option` により「存在しない」状態を表現している。
  - `TickDriverRuntime` は provision 後の資産であり、SystemState の `Drop` で `Option::take()` して shutdown している。
  - テスト用 `ManualTestDriver` は `cfg(any(test, feature = "test-support"))` 下にあり、`SchedulerConfig.runner_api_enabled()` が `true` でないと provision できない（現状は `install_scheduler_and_tick_driver_from_config` で ManualTest のときだけ runner API を有効化する特例がある）。
- **RemotingConfig の二重管理**: `PathIdentity` が canonical host/port と quarantine duration を保持している一方で、同等情報を `remoting_config: Option<RemotingConfig>` として別途保存している（`apply_actor_system_config` が両方を更新）。
- **RemoteWatchHook の表現**: `RemoteWatchHook` は `&mut self` を要求するため、SystemState は mutex で保護しているが、未登録状態は `Option` で表現している。hook が無い場合は watch/unwatch のフォールバック経路が動く。

## 2. 要件に対するギャップ（Requirement-to-Asset Map）
### 要件1: SystemStateGeneric の未初期化状態排除
- scheduler_context/tick_driver_runtime が `Option` → **Missing**（`system_state.rs`）。
- `install_*` による後差し込みが前提 → **Missing**（`system_state.rs`, `system_state_shared.rs`, `base.rs`）。
- remote_watch_hook が `Option` → **Missing**（`system_state.rs`）。※振る舞い（未登録ならフォールバック継続）はすでに存在するが、表現が要件と不一致。
- remoting_config が `Option` かつ PathIdentity と二重 → **Constraint/要整理**（既存互換と “remoting 有効/無効” の定義を固める必要）。

### 要件2: 初期化順序の保証（構築フロー）
- `new_with_config_and()` が `new_empty()` → `apply_*` → `install_*` の順で組み立てている → **Missing**（初期化順序が逆転している）。
- SystemStateGeneric 生成後に構成・依存物を注入する API が存在し、利用されている → **Missing**（少なくともプロダクション経路では不要化が必要）。

### 要件3: SystemStateSharedGeneric による共有アクセス維持
- `SystemStateSharedGeneric::scheduler_context()` / `tick_driver_runtime()` が `Option` を返す → **Missing**（呼び出し側が分岐/expect を持つ）。
- 影響範囲: `modules/actor/src/core/system/base.rs`, `modules/actor/src/core/typed/system.rs`, `modules/actor/src/std/system/base.rs` ほか（`scheduler_context().is_some()` や `expect()` が点在） → **Unknown/要棚卸し**（API を非 Option 化した場合の移行方針）。

### 要件4: テスト支援としての new_empty() 維持
- `new_empty()` がテスト専用ではない → **Missing**（`core` と `std` の両方）。
- `new_empty()` が返す SystemState が未初期化（scheduler/tick driver 無し） → **Missing**（要件では完全初期化済みが必要）。

### 要件5: 回帰防止と品質ゲート
- `ci-check.sh all` 完走は未確認 → **Unknown**（実装後の検証が必要）。
- core への `cfg(feature="std")` 禁止、lint 回避禁止 → **Constraint**（現行ガイドラインに一致。設計/実装時に逸脱させない）。

## 3. 実装アプローチ案
### Option A: 既存構造を拡張（最小変更で Option 排除）
**狙い**: SystemStateGeneric の設計を大きく崩さず、問題の 4 フィールドだけ「常に初期化済み」を保証する。

- 変更の中心:
  - `SystemStateGeneric::new()` を「完全初期化済み」を返すファクトリ（または `new_*` を新設）に変更し、`scheduler_context` と `tick_driver_runtime` を必須引数（または内部で構築）にする。
  - remote_watch_hook は `Option` を廃止し、未登録時は Noop 実装（常に `false`）を保持する。
  - remoting_config は `PathIdentity` を単一ソースにして導出する（フィールド削除、もしくは二重管理をやめる）。
  - `ActorSystemGeneric::new_with_config_and()` は `new_empty()` に依存せず、構成と依存物を揃えた状態で SystemState を生成してから bootstrap する。
  - `new_empty()` は `cfg(any(test, feature = "test-support"))` に限定し、デフォルトの scheduler/tick driver（例: ManualTestDriver + runner API 有効）で完全初期化済み SystemState を返す。
- **利点**: 変更範囲を system/base 周辺に寄せられ、最短で要件を満たしやすい。
- **懸念**: SystemStateGeneric の “内部ロック多数” という現状設計は温存され、`docs/guides/shared_vs_handle.md` が推奨する「ロジック本体は `&mut self`、共有は外側でラップ」とは完全一致しない。

### Option B: 新コンポーネント導入（初期化専用 Builder / Handle を新設）
**狙い**: 初期化順序と責務分離を型/構造で強制し、今後の “shared vs handle” 方針に寄せる。

- 例:
  - `SystemStateBuilderGeneric`（仮）を新設し、`event_stream → path_identity → scheduler_context → tick_driver_runtime → system_state` の順に組み立てる API を提供する。
  - remote watch hook / tick driver runtime のライフサイクル（shutdown を含む）を `SystemRuntimeHandle` のような別型に閉じ込め、SystemState は参照だけ持つ。
- **利点**: 初期化順序の誤りを構造で防ぎ、将来の refactor（内部可変性の集約）にも繋げやすい。
- **懸念**: 新しい型/ファイル追加が増え、既存の生成 API とテストの組み替え量が増える（短期の変更量は Option A より大きい）。

### Option C: ハイブリッド（段階導入）
**狙い**: まずは要件を満たす最小修正で安定化し、その後に設計原則へ寄せる。

- フェーズ1: Option A 相当で Option 排除と初期化順序を是正（CI を通す）。
- フェーズ2: 別 spec で「SystemStateGeneric の内部可変性を外側へ移す/`*Shared` の必要性を再評価する」大きめの整理を実施。
- **利点**: 回帰リスクを抑えつつ前進できる。
- **懸念**: 一時的に “過渡期 API/構造” が残りやすい（短期間の負債を許容する判断が必要）。

## 4. 努力度・リスク
- 努力度: **L (1–2週間)** — 生成フローの組み替え（core/std/typed）と `new_empty()` の再定義、テストの大量修正、Drop/shutdown の整理が必要。
- リスク: **中** — 初期化順序・tick driver shutdown・remoting 有効判定の変更は起動/停止/監視系に波及しやすい。テストでの回帰検出が鍵になる。

## 5. Research Needed（設計フェーズへ持ち越す調査項目）
- scheduler_context/tick_driver_runtime を “常に存在” としたとき、既存の `Option` 依存箇所（`is_some/expect/map`）をどの API 変更で吸収するか（core/typed/std の境界も含む）。
- `new_empty()` のデフォルト tick driver を何にするか（ManualTestDriver を採用する場合、runner API をどこで必ず有効化するか）。
- remoting_config を PathIdentity から導出する際の「remoting 有効/無効」の定義（canonical_host が None のときのみ None にする等）と、remote/cluster 拡張が期待する挙動互換。
- remote watch hook を Noop 化した場合の登録/差し替えポリシー（複数登録を許すか、最後が勝つか、起動後変更を許すか）。
- `SystemStateGeneric` を将来的に `&mut self` ベースへ寄せる場合、ActorCell/ActorRef などの呼び出し形態（現状 `ArcShared<SystemStateGeneric>` 前提）をどう移行するか。

---
分析はギャップ抽出と選択肢提示に留めており、実装方針の最終決定は設計フェーズで行う。

