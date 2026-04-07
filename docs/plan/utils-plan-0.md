fraktor-rs 同期プリミティブ抽象層の問題分析

対象リポジトリ

- リポジトリ: fraktor-rs(Rust 製アクターフレームワーク、Apache Pekko / protoactor-go を参照実装とする)
- 対象 crate: modules/utils/ (fraktor-utils-rs) が同期プリミティブを定義、他 crate から利用される
- プロジェクト価値観: YAGNI, Less is more, 後方互換不要(リリース前)
- 関連規約: .agents/rules/rust/immutability-policy.md, .agents/rules/rust/module-structure.md

背景

Debug ビルド時のみ Mutex の再入(同一スレッドが同じロックを再度取得する誤り)を検出する仕組みを導入したい。AI 生成コードで再入を誘発するロック呼び出しが混入しやすいため、ガードレールとして機能させたい。

実装着手前に既存の同期プリミティブ抽象を調査したところ、4層の問題が積み重なっていることが判明した。

現状の型階層

fraktor-utils-rs (modules/utils/)
├── core/sync/sync_mutex_like.rs
│   └── trait SyncMutexLike<T>              ← 抽象インタフェース
│       └── impl for SpinSyncMutex<T>       ← no_std 実装(spin::Mutex ラッパー)
├── std/sync_mutex.rs
│   └── impl SyncMutexLike for StdSyncMutex<T>  ← std 実装(std::sync::Mutex ラッパー、feature = "std" 時のみ)
├── core/sync/runtime_lock_alias.rs
│   └── pub type RuntimeMutex<T> = RuntimeMutexBackend<T>
│       where RuntimeMutexBackend<T> = StdSyncMutex<T> if feature "std"
│                                    = SpinSyncMutex<T> otherwise
└── core/collections/queue/
├── sync_queue_shared.rs
│   └── pub struct SyncQueueShared<T, K, B, M = SpinSyncMutex<SyncQueue<T,K,B>>>
│         where M: SyncMutexLike<SyncQueue<T,K,B>>
├── sync_mpsc_producer_shared.rs   (同様のジェネリック)
├── sync_mpsc_consumer_shared.rs   (同様)
├── sync_spsc_producer_shared.rs   (同様)
└── sync_spsc_consumer_shared.rs   (同様)

(RwLock 側も同じ構造: SyncRwLockLike, SpinSyncRwLock, StdSyncRwLock, RuntimeRwLock)

問題1: 抽象と実態の乖離(RuntimeMutex が事実上無視されている)

事実

- RuntimeMutex は RuntimeMutexBackend を経由して feature によって StdSyncMutex / SpinSyncMutex を切り替える「統一インタフェース」として設計されている
- プロジェクト内 RuntimeMutex の利用箇所: 100+ ファイル
- しかし SpinSyncMutex を直接参照しているファイル: 44 件 が並存している
  - 内訳: stream-core 21、actor-core 3、remote-* 3、utils 内部 17(定義・テスト含む)
- 使い方は全て ArcShared<SpinSyncMutex<T>> または SpinSyncMutex<Option<T>> という単純な形で、SpinSyncMutex 固有 API(例: as_inner())にはほぼ依存していない

決定的な規約との不整合

プロジェクト規約 .agents/rules/rust/immutability-policy.md の AShared パターン記述:

▎ inner に ArcShared<SpinSyncMutex<A>> を保持する AShared 構造体を新設

規約自体が SpinSyncMutex を指名しており、RuntimeMutex への言及が存在しない。つまり設計意図として RuntimeMutex 抽象は最初から一級市民ではなく、規約と実装のどちらを信じればよいかが曖昧なまま走っている。

影響

- 仮に RuntimeMutex 経路に DebugMutex を仕込んでも、44ファイルがバイパスするのでガードレールとして機能しない
- 新規コードを書く際に「どちらを使うべきか」の指針が欠落

エビデンス

- modules/remote-core/src/lib.rs:87: //!   `SpinSyncMutex` + `&self`) is forbidden in this crate and pushed to adapters.
- この doc コメントは「spin 直接は禁止」を謳っているが、他の crate では遵守されていない

  ---
問題2: SyncQueueShared ファミリーが完全なデッドコード

事実

modules/utils/src/core/collections/queue/ 配下の以下 8 ファイルが SyncQueueShared ファミリーを構成している:

sync_queue_shared.rs              (pub struct SyncQueueShared<T,K,B,M>)
sync_mpsc_producer_shared.rs
sync_mpsc_consumer_shared.rs
sync_spsc_producer_shared.rs
sync_spsc_consumer_shared.rs
sync_spsc_producer_shared/tests.rs
tests.rs
queue.rs                          (pub use で公開)

modules/utils/src/core/collections/queue.rs:

pub use sync_queue_shared::{
SyncFifoQueueShared, SyncMpscQueueShared, SyncPriorityQueueShared,
SyncQueueShared, SyncSpscQueueShared,
};
pub use sync_mpsc_consumer_shared::SyncMpscConsumerShared;
pub use sync_mpsc_producer_shared::SyncMpscProducerShared;
pub use sync_spsc_consumer_shared::SyncSpscConsumerShared;
pub use sync_spsc_producer_shared::SyncSpscProducerShared;

pub use されているにもかかわらず、workspace 全体で grep したところ利用者はゼロ(utils 内の自己参照とテストを除く)。

# 検証コマンド
rg "SyncQueueShared|SyncMpscProducerShared|SyncMpscConsumerShared|SyncSpscProducerShared|SyncSpscConsumerShared" modules/
# → 8 files, 全て modules/utils/src/core/collections/queue/ 配下

仮説: 典型的な "forked overengineering"

- actor-core のメールボックスキュー(本来この抽象の想定ユーザー)は独自実装で RuntimeMutex + 直接キュー実装を使っており、SyncQueueShared 経路を通っていない
- 「メールボックス用に作ったが、結局別の方法で実装された」という典型的なパターンの残骸と思われる

影響

- サイズが大きい(~1000 行オーダー)のに誰も使っていない死荷重
- 「公開 API なので消せない」という後方互換バイアスを発生させる原因
- 後述する SyncMutexLike trait の存在意義の唯一の根拠になっている(問題 3 で詳述)

  ---
問題3: SyncMutexLike trait の存在意義がほぼ消失

事実

SyncMutexLike trait は2種類の使われ方をしている:

(a) ジェネリック境界としての使用
pub struct SyncQueueShared<T, K, B, M = SpinSyncMutex<SyncQueue<T, K, B>>>
where M: SyncMutexLike<SyncQueue<T, K, B>>, { ... }
この使い方は SyncQueueShared ファミリー 8ファイルにしか存在しない。

(b) trait method を呼ぶための import
actor-core の *_shared.rs 系で RuntimeRwLock<T> に対して .write() / .read() を呼ぶために use SyncRwLockLike が必要、という形でのみ登場。これはジェネリック抽象としての利用ではなく、単に trait method の可視化のため。

(c) プロジェクト規約による誘導
各 crate の clippy.toml に以下のエントリがある:
{ path = "std::sync::Mutex",
reason = "Use impl of SyncMutexLike within production code",
replacement = "fraktor_utils_core_rs::sync::SyncMutexLike" }
std::sync::Mutex の直接使用を禁止して SyncMutexLike impl に誘導する意図。

問題

- 問題 2 で SyncQueueShared がデッドコードと判明したため、(a) の唯一の実利用箇所が無価値化
- プロダクションコードで SyncMutexLike<T> のジェネリック境界を実用している箇所はゼロになる
- trait の impl は SpinSyncMutex と StdSyncMutex の 2 つだが、後述する問題 4 により StdSyncMutex も実需ゼロ
- 1 実装しかない trait は YAGNI 原則上、幽霊抽象

影響

- trait を残すと「抽象のための抽象」が恒常化
- DebugMutex 導入時に「trait 経由で差し替えるべきか、具体型に埋め込むべきか」という不要な選択を生む

  ---
問題4: StdSyncMutex / StdSyncRwLock の存在意義もほぼ消失

事実

- StdSyncMutex は std::sync::Mutex のラッパー(modules/utils/src/std/sync_mutex.rs)
- StdSyncMutex を直接参照しているファイル: 6 件、すべて utils/src/std/ 配下の定義とテスト
- プロダクションコードで std::sync::Mutex 固有のセマンティクス(poisoning, OS parking による長時間ブロッキング時の効率)に依存している箇所は ゼロ
- RuntimeMutex 経由で実際に StdSyncMutex が選択されるかは feature flag 次第だが、アクターフレームワークの性質上 lock 保持時間は短く、spin lock で十分

SpinSyncMutex の実際のバックエンド

modules/utils/Cargo.toml:
spin = { workspace = true, default-features = false,
features = ["mutex", "spin_mutex", "rwlock", "portable_atomic"] }
portable-atomic = { workspace = true, default-features = false,
features = ["critical-section"] }
portable-atomic + critical-section 構成により、atomics 非搭載の embedded ターゲットまで含めて spin::Mutex が動作する。つまり no_std / std を問わず SpinSyncMutex 1 本で全ターゲットをカバーできる。

影響

- StdSyncMutex を維持する技術的理由がない
- RuntimeMutex の存在意義の根拠(「feature に応じて最適な Mutex を選ぶ」)が崩壊する
- modules/utils/src/std/sync_mutex* (複数ファイル)が死荷重

  ---
問題の構造まとめ

4 つの問題は独立ではなく、1 つの根本原因から派生した連鎖 である:

[根本] 抽象化コストに見合う実需が存在しない
│
├─→ 問題4: StdSyncMutex は OS parking 等の std 固有挙動が使われていない
│          → RuntimeMutex の feature 切替に意味がない
│
├─→ 問題3: SyncMutexLike trait は 1 実装しかない幽霊抽象
│          (問題2でジェネリック境界の唯一の実用途も消える)
│
├─→ 問題2: SyncQueueShared ファミリーが誰にも使われていないデッドコード
│          (trait のジェネリクスを活かす想定だったが未接続)
│
└─→ 問題1: 規約自体が SpinSyncMutex を指名しているため、
RuntimeMutex 抽象が守られず 44 ファイル漏洩

言い換えると、utils crate が将来の拡張性を見越して4層の抽象を構築したが、実際にはどの層にも本物の需要がなく、使う側は最下層(SpinSyncMutex)を直接掴みに行っている、という状態。

提案される方向性(決定は未)

選択肢 A: 完全一本化(推奨)

削除:
- RuntimeMutex / RuntimeRwLock 型エイリアス
- StdSyncMutex / StdSyncRwLock 具体型
- SyncMutexLike / SyncRwLockLike trait
- SyncQueueShared ファミリー 8ファイル
- modules/utils/src/std/sync_mutex* 関連
- SyncQueueShared 系のジェネリック M パラメータ

残す:
- SpinSyncMutex / SpinSyncRwLock を唯一の具体型として一本化

効果:
- 規約(SpinSyncMutex を指名)と実装が一致する
- 44 ファイルの「漏洩」は事後的に正当化される(逆に 100+ ファイルの RuntimeMutex を SpinSyncMutex に書き換える機械置換が発生)
- DebugMutex ガードレールは SpinSyncMutex 内部の #[cfg(debug_assertions)] 分岐に 1 箇所集約できる
- 将来「std::Mutex の OS parking が必要」なケースが本当に発生したら、その時点で再抽象化すればよい(YAGNI 原則)

選択肢 B: trait だけ残す

- 具体型は SpinSyncMutex 一本だが SyncMutexLike trait は保持
- 「1 実装しかない trait」問題が残るため中途半端

選択肢 C: 現状維持 + 漏洩 cleanup のみ

- 44 ファイルを RuntimeMutex に統一するだけ
- 根本原因(規約と抽象の不整合、デッドコード、単一実装の trait / 具体型)は温存
- DebugMutex 導入時に再度同じ問題に直面する

段階 PR 案(選択肢 A 採用時)

┌─────┬─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬──────────┐
│ PR  │                                                                    内容                                                                     │  独立性  │
├─────┼─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┼──────────┤
│ PR1 │ SyncQueueShared ファミリー 8ファイル削除(純粋なデッドコード削除)                                                                            │ 完全独立 │
├─────┼─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┼──────────┤
│ PR2 │ RuntimeMutex / RuntimeRwLock → SpinSyncMutex / SpinSyncRwLock に機械置換(100+ ファイル)                                                     │ PR1 後   │
├─────┼─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┼──────────┤
│ PR3 │ StdSyncMutex / StdSyncRwLock / SyncMutexLike / SyncRwLockLike / utils/src/std/sync_* 削除、clippy.toml の disallowed-types replacement 更新 │ PR2 後   │
├─────┼─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┼──────────┤
│ PR4 │ DebugMutex(debug-only 再入検出)を SpinSyncMutex 内部に #[cfg(all(debug_assertions, feature = "std"))] で埋め込み                            │ PR3 後   │
└─────┴─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴──────────┘

検討事項(他の AI に判断を仰ぎたい論点)

1. SyncQueueShared ファミリーは意図的に公開 API として残されているか、それとも忘れられた残骸か
   - fraktor-utils-rs は crates.io 公開想定の crate description を持つ
   - ただし workspace 内で使われていない以上、「外部ユーザーのための仮想的な API」として温存することは YAGNI 違反と考える
   - git log で追加経緯を確認する価値はある
2. SyncMutexLike trait を完全削除するか、SpinSyncMutex の inherent method のみに寄せるか
   - trait を残す派の論拠: 将来の差し替え余地
   - 削除派の論拠: 1 実装しかない trait は幽霊抽象
   - プロジェクトの YAGNI 原則では削除派が優勢
3. 選択肢 A の段階的移行中の整合性
   - PR2(機械置換)と PR3(型削除)の間は両方の型が共存する過渡状態になる
   - 合理的な中間状態を保てるか、一括でやるべきか
4. DebugMutex の検出機構
   - 再入検出の原理: try_lock + 呼び出しスレッドの識別子(AtomicU64 に ThreadId のハッシュを保存)を用い、try_lock 失敗時に holder == self ならパニック
   - std::thread::ThreadId は std 必須なので、検出は #[cfg(all(debug_assertions, feature = "std"))] 限定になる
   - no_std debug ビルドでは検出されないが、テストは通常 std 上で走るためガードレールとして十分
5. RwLock 側の対称性
   - Mutex と同じ問題構造を持つ(SyncRwLockLike, SpinSyncRwLock, StdSyncRwLock, RuntimeRwLock)
   - 同じ PR セットで同時に整理すべきか、別 PR セットに分けるべきか

確認に使った主要ファイル

- modules/utils/src/lib.rs (RuntimeMutexBackend の cfg 分岐定義)
- modules/utils/src/core/sync/runtime_lock_alias.rs
- modules/utils/src/core/sync/sync_mutex_like.rs
- modules/utils/src/core/sync/sync_mutex_like/spin_sync_mutex.rs
- modules/utils/src/std/sync_mutex.rs
- modules/utils/src/core/collections/queue.rs
- modules/utils/src/core/collections/queue/sync_queue_shared.rs
- modules/utils/clippy.toml
- modules/remote-core/src/lib.rs:87 (SpinSyncMutex 禁止の doc コメント)
- .agents/rules/rust/immutability-policy.md (AShared パターン規約)
- modules/utils/Cargo.toml (spin + portable-atomic + critical-section 構成)
