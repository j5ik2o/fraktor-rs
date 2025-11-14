# 設計ドキュメントテンプレート

---
**分量ガイド**: 最大 1000 行

**目的**: 実装者間で設計解釈のブレを防ぎ、誰が読んでも同じ結果に到達できる情報を提供する。

**基本方針**:
- 実装判断に直結する必須情報のみを書く
- 重要箇所は文章より図表や表で簡潔に伝える
- 機能の複雑さに応じて深さを調整する

**警告**: 1000 行に近づいたら機能分割や簡素化を再検討すること。
---

## 概要
2〜3 段落で完結に記述する。
- **Purpose**: この機能がどのユーザ／ユースケースにどんな価値を届けるか
- **Users**: 代表的な利用者と利用シナリオ
- **Impact**: 既存システムに与える変化（あれば）

### 目標 (Goals)
- 主要な成果1
- 主要な成果2
- 成功判定条件

### 非目標 (Non-Goals)
- 今回のスコープから外す事項
- 後続検討に回す項目
- 他システムへの波及を明示

## アーキテクチャ

### 既存アーキテクチャの把握
変更対象が既存システムの場合のみ記述。
- 現在のパターン／制約
- 守るべきドメイン境界
- 維持すべき統合ポイント
- 併せて解消する技術的負債

### ハイレベルアーキテクチャ
- Mermaid などで構成図を推奨（複雑機能では必須）
- 既存パターンをどこまで維持するか
- 新規コンポーネント追加理由
- 技術スタックとの整合性
- Steering の原則をどう満たすか

### 技術スタック / 設計判断
- **新規機能**: 採用技術と理由、比較した代替案を記述
- **既存拡張**: 既存スタックとの整合性、新規依存、既存ルールからの逸脱理由を記述

#### 主要設計判断
最大 1〜3 件。フォーマット:
- **Decision**: 技術的な決定事項
- **Context**: 背景・課題
- **Alternatives**: 検討した他案
- **Selected Approach**: 採用内容と仕組み
- **Rationale**: 採用理由
- **Trade-offs**: 得られるもの／失うもの

## システムフロー
複雑なフローがある場合のみ図示。
- シーケンス図: 複数コンポーネント間のイベントや API 呼び出し
- プロセスフロー: 分岐や状態遷移
- データフロー: 変換や ETL
- 状態図: 状態遷移が複雑な場合
- イベントフロー: 非同期・イベント駆動

## API ブループリント
追加・変更されるインターフェイスの骨組みを提示する。

### 型・トレイト一覧
- 公開／内部を問わず追加・変更される `trait / struct / enum / type alias` を列挙
- 責務と可視性（例: `pub(crate)`）を併記
- 命名規約や lint 制約があれば明記

### シグネチャ スケッチ
```rust
pub enum ExampleCategory {
  Realtime { backlog: NonZeroUsize },
  Priority { comparator: PriorityRule },
}

pub struct ExampleFront;
impl ExampleFront {
  pub fn build<T>(&self, category: ExampleCategory) -> ExampleHandle<T>;
  pub fn register_backend(&mut self, key: BackendKey, factory: BackendFactory);
}
```
- 本文は書かずシグネチャのみ
- ジェネリクス境界／ライフタイム／戻り値／エラー型を明記

## クラス／モジュール図
- Mermaid または PlantUML で主要コンポーネントと依存方向を図示
- 変更前後の差分は色や凡例で区別
- 階層構造（例: front → backend → storage）がある場合は矢印で示す

```mermaid
classDiagram
  class QueueFront {
    +build(category)
    +register_backend()
  }
  QueueFront --> QueueCategory
  QueueFront --> QueueHandle
  QueueHandle --> Backend (<<trait>>)
```

## クイックスタート / 利用例
- 10〜20 行程度で代表シナリオを記述
- カテゴリ選択 → インスタンス生成 → 主要操作の順で示す
- doctest / テスト形式にして、後で guides へ転用できるようにする

```rust
fn mailbox_queue_setup() {
  let front = QueueFront::default();
  let queue = front.build::<SystemMessage>(QueueCategory::Bounded { capacity: 64 });
  queue.offer(message).unwrap();
}
```

## 旧→新 API 対応表

| 旧 API / 型 | 新 API / 型 | 置換手順 | 備考 |
| --- | --- | --- | --- |
| `VecRingBackend<T>` を直接 new | `QueueFront::build(QueueCategory::Realtime)` | 呼び出し側から backend 依存を排除し、QueueFront の戻り値を保持 | オーバーフロー制御はカテゴリで指定 |
| `BinaryHeap<TaskRunEntry>` | `QueueFront::build(QueueCategory::Priority { order: TaskOrder })` | 優先度制御を backend コンフィグへ委譲 | `peek_min` は Priority ハンドル経由 |

## 要件トレーサビリティ
複雑な機能のみ記述。EARS 要件との対応表を作成。

| 要件ID | 要約 | 実装コンポーネント | インターフェイス | 参照フロー |
| --- | --- | --- | --- | --- |
| 1.1 | ～ | QueueFront | build() | sequence#1 |

## コンポーネント & インターフェイス
ドメイン／レイヤ単位で小見出しを設置し、以下を記述:

### [ドメイン/レイヤ名]
- 責務
- 入出力
- 依存関係（Inbound/Outbound/External）
- 外部依存の調査結果（必要なら WebSearch/WebFetch で裏付け）

#### 契約定義 (必要なもののみ採用)

**Component Interface**（業務ロジック向け）
```rust
pub trait ComponentName {
  /// 明確な入出力とエラー型を伴うシグネチャ
  fn method_name(&self, input: InputType) -> Result<OutputType, ErrorType>;
}
```
- 前提条件 / 事後条件 / 不変条件を列挙

**API Contract**（REST/GraphQL）
| メソッド | エンドポイント | リクエスト | レスポンス | エラー |
| --- | --- | --- | --- | --- |
| POST | /api/resource | CreateRequest | Resource | 400, 409, 500 |

**Event Contract**（イベント駆動）
- 発行イベント: 名前、スキーマ、トリガ条件
- 購読イベント: 名前、処理方針、冪等性

### ドメインモデル
- 集約 / エンティティ / 値オブジェクト / ドメインイベント
- ビジネスルールと不変条件
- 関係が複雑なら Mermaid 図も検討

## データモデル
必要な層のみ記述。

### 論理データモデル
- エンティティ、属性、関連、キー
- トランザクション境界、参照整合性

### 物理データモデル
ストレージ種別ごとに必要事項を記述（RDB, Document, Event Store 等）。

### データ契約 / 連携
- API スキーマ、バリデーション、シリアライズ
- イベントスキーマとバージョン戦略
- クロスサービスの同期・整合性戦略

## エラーハンドリング

### エラーストラテジ
- 各エラータイプの処理パターンと復旧方法

### エラー分類と応答
- ユーザエラー (4xx)
- システムエラー (5xx)
- ビジネスロジックエラー (422 等)
- 複雑な場合はフローチャートで可視化

### モニタリング
- ログ、トレース、メトリクスで何を監視するか

## テスト戦略
- ユニットテスト: 中核機能 3〜5 件
- 統合テスト: クロスコンポーネント 3〜5 フロー
- E2E/UI テスト (必要に応じて)
- パフォーマンス/負荷テスト (該当時)

## 追加セクション（必要時のみ）

### セキュリティ
- 脅威モデル、認証/認可、データ保護、コンプライアンス

### パフォーマンス & スケーラビリティ
- 目標指標、計測手段、スケール戦略、キャッシュ方針

### 移行戦略
- Mermaid フローでフェーズとロールバック条件を図示
- フェーズ分解、検証ポイント、リスク緩和策
