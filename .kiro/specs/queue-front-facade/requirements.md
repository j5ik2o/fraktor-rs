# 要件ドキュメント

## プロジェクト説明 (入力)
  目的:
    utils-core のコレクションライブラリに統一的なフロント API を導入し、
    enum で用途カテゴリを選ぶだけで内部 backend が決定される仕組みを作る。
    既存の Queue/Stack/Wait 実装を段階的に移行し、*Backend を外部に公開しない。

  背景と課題:
    - queue/backend 以下が pub のまま露出し、AI/人間とも backend を直接利用してしまう。
    - Scheduler/Diagnostics/TickFeed などが VecDeque や BinaryHeap を直接使い始め、
      モジュール横断で統一感が失われている。
    - crossbeam は no_std 非対応のため採用できず、heapless/bbqueue などを backend に
      取り込む必要がある。

  スコープ:
    - utils-core/collections/queue を中心に、カテゴリ別フロント (Realtime, Bounded, Priority 等) を追加。
    - enum HogeType (仮称) で用途を選ぶ → pub(crate) HogeBackend が内部で決定される流れを実装。
    - 既存 *Backend トレイト/構造体を pub(crate) 化し、公開 API はフロント層に限定。
    - docs/steering に「バックエンド直接利用禁止」「新規キューはフロント経由」の方針を追記。
    - 代表的な既存利用箇所 (Scheduler Diagnostics, TaskRunQueue, TickFeed) を新フロントへ移行。

  非スコープ:
    - 完全な再設計 (例: Queue API をゼロから作り直す)。
    - queue 以外のコレクション (stack/wait) の大幅変更。ただし必要な場合は follow-up issue 化。

  成果物:
    - 新フロント API 実装とテスト。
    - 既存コードの移行 (最低でも Scheduler/Diagnostics/TickFeed)。
    - ドキュメント/steering 更新 (使用ルールと移行ガイド)。
    - 将来の backend 追加 (heapless/bbqueue 等) を見据えた設計解説。

## 要件

### 要件1: カテゴリ駆動のフロント API
**Objective:** ランタイム利用者として、用途カテゴリを選ぶだけで適切なキュー機能を得たい。なぜならバックエンド知識なしで一貫した API を使いたいからである。

#### 受け入れ条件
1. When ランタイム利用者が `QueueCategory::Realtime` でキュー生成を要求すると、Queue Front API shall Realtime 向け内部バックエンドを構築し、フロント型のハンドルだけを返す。
2. When 利用者が Bounded など容量指定カテゴリを選択すると、Queue Front API shall 受け取った容量制約を対応する内部バックエンドへ伝播し、公開型には容量情報だけを露出する。
3. If 利用者が enum で未定義のカテゴリを指定した場合、Queue Front API shall コンパイル時または初期化時に明示的なエラーを提示し、内部バックエンドを作成しない。
4. While フロント API で生成したキューが稼働している間、Queue Front API shall enqueue/dequeue 操作を統一トレイト経由で委譲し、バックエンド固有の型を公開面に現さない。

### 要件2: バックエンド非公開化と方針遵守
**Objective:** モジュール保守者として、バックエンドを pub(crate) 以下に封じ込めたい。なぜなら方針違反の直接利用を防ぎたいからである。

#### 受け入れ条件
1. When `utils-core/collections/queue/backend` 配下にバックエンド実装を追加すると、Build Pipeline shall その可視性が `pub(crate)` 以下であることを検証し、違反時に失敗させる。
2. When API ドキュメントを生成すると、Steering Documentation Process shall 「バックエンド直接利用禁止」と「フロント経由利用」の章を公開し、利用例を最新化する。
3. If Linting で `*Backend` 型の `pub` 再エクスポートが検出された場合、CI Pipeline shall チェックを失敗させ、違反モジュールをレポートする。
4. Where actor-core や diagnostics など依存クレートがキュー機能を利用する場合、それらのクレート shall フロントファサードとカテゴリ enum のみを import し、バックエンドモジュールを参照しない。

### 要件3: 代表モジュールの移行完了
**Objective:** ランタイム保守者として、Scheduler/Diagnostics/TickFeed を新フロントへ移行したい。なぜならモジュール横断の統一感を回復したいからである。

#### 受け入れ条件
1. When Scheduler が `TaskRunQueue` を初期化すると、Scheduler Module shall Queue Front API を介して Bounded もしくは Realtime カテゴリのキューを生成する。
2. When Diagnostics がキュー負荷や滞留長を収集すると、Diagnostics Module shall Queue Front API から観測ハンドルを取得し、バックエンド内部状態を直接参照しない。
3. When TickFeed が優先度付きキューを要求すると、TickFeed Module shall Queue Front API 経由で Priority カテゴリを生成し、その比較規則に従う。
4. If これらのモジュールが VecDeque や BinaryHeap を直接生成しようとした場合、Migration Tests shall 失敗し、フロント API 利用を促すメッセージを出力する。

### 要件4: バックエンド拡張と no_std 互換性
**Objective:** プラットフォームオーナーとして、heapless/bbqueue など将来の backend を簡単に追加したい。なぜなら no_std/STD の双方で一貫した API を維持したいからである。

#### 受け入れ条件
1. When heapless や bbqueue 由来の新 backend を導入すると、Queue Front API shall カテゴリエントリを追加するだけで選択できるようにし、既存呼び出し箇所の変更を不要にする。
2. While no_std ターゲット向けにビルドしている間、Queue Front API shall no_std 制約を満たす backend だけを自動選択し、STD 依存 backend をリンクしない。
3. When STD ターゲット向けにビルドすると、Queue Front API shall STD 向け backend を特徴フラグなしで利用可能にし、ランタイム本体に `#[cfg(feature = "std")]` を追加しない。
4. If カテゴリによって利用可能な backend 能力が異なる場合、Queue Front API shall Steering ドキュメントに可用性マトリクスを記述し、今後の移行計画に反映できるようにする。
