# Brief: cluster-singleton-settings-contract

## Problem

Cluster Singleton（cluster 全体で 1 つだけ動く actor を保証する機構）に関する語彙が fraktor-rs に一切存在しない。Phase 3 で singleton manager / proxy の runtime を実装する際、設定・検証・join 互換性の契約が先に固まっていないと、runtime 実装と設定設計が同じ PR に混ざり review scope が肥大する。gap analysis カテゴリ6 の easy 項目（設定群 6 概念）が対象。

## Current State

対応モジュールなし。`ClusterExtensionConfig` の validation と `ClusterCompatibilityKeyCatalog` / `JoinCompatibilityComposition` による join compatibility 合成という、設定契約を受け入れる基盤は実装済み（FailureDetectorConfig が同型の先行例）。

## Desired Outcome

- classic / typed 双方の singleton 設定契約（Pekko の `ClusterSingletonManagerSettings`（classic / typed）、`ClusterSingletonProxySettings`、typed `ClusterSingletonSettings` に相当: role、removal margin、handover retry、buffer size、lease 設定スロット等）が型として定義される。
- 設定は install / start 境界の Cluster Configuration Validation で不成立値が拒否され、join compatibility key として mismatch reason を生成する（FailureDetectorConfig と同じパターン）。
- `ClusterSingletonSetup` 相当の ActorSystem setup 統合点が定義される。
- handover が進まない状態を検知する `ClusterSingletonManagerIsStuck` 相当の観測契約（検知条件と通知の型）が定義される。

## Approach

`configure-cluster-failure-detector` で確立した「設定型 → validation → join compatibility key → std factory 接続」のパターンを singleton 設定に適用する。runtime（manager / proxy の状態機械）は実装せず、設定・検証・互換性・観測契約までで spec を閉じる。これにより Phase 3 の runtime spec は契約を参照するだけで済む。

## Scope

- **In**:
  - singleton manager / proxy / typed singleton の設定型一式（core/config 相当の配置）
  - 設定の validation（install / start 境界での拒否）
  - join compatibility key への組み込み（`ClusterCompatibilityKeyCatalog` / `JoinCompatibilityComposition` 拡張）
  - `ClusterSingletonSetup` 相当の setup 統合点
  - stuck 検知の契約（条件・閾値・通知型）
- **Out**:
  - `ClusterSingletonManager` の oldest-election / handover 状態機械（Phase 3 / hard）
  - `ClusterSingletonProxy` の location 追跡・buffering runtime（Phase 2-3 / medium）
  - lease backend の実装（cluster-downing 系の concrete lease backend と共通の Phase 3 項目）

## Boundary Candidates

- 設定型（pure data）と validation / compatibility 接続（extension 統合）の分離
- classic 設定と typed 設定の対応関係（typed が classic を内包する Pekko 構造を Rust でどう表すか）

## Out of Boundary

- singleton の配置決定ロジック（membership ordering 契約は cluster-membership-event-surface が所有）
- coordinated shutdown との handover 連携（runtime spec の領域）

## Upstream / Downstream

- **Upstream**: cluster-active-compatibility-baseline（完了済み、join compatibility 基盤）、configure-cluster-failure-detector（完了済み、設定契約パターンの先行例）
- **Downstream**: Phase 3 の singleton manager / proxy runtime spec、module setup integration（本 spec の setup 統合点を使う）

## Existing Spec Touchpoints

- **Extends**: cluster-active-compatibility-baseline の compatibility key catalog を拡張する（spec 自体は新規）
- **Adjacent**: configure-cluster-failure-detector（同じ設定パターンを共有。実装を流用するが spec 境界は分離）

## Constraints

- `cluster-core-kernel` の `no_std` 境界を維持。設定型・validation・compatibility は core、host 依存があれば std へ。
- 「設定だけ存在して配線されない」状態を作らないこと。validation / join compatibility / setup 統合まで含めて初めて完了とする（ai-antipattern: 未使用コード検出の回避）。
- 命名は Pekko のドメイン用語（`SingletonManagerSettings` 等）を優先する（reference-implementation 命名規約）。
