# 実装計画

- [x] 1. 基盤
- [x] 1.1 (P) VersionVector に観測差分プリミティブを追加する
  - self の dot のうち相手 version vector に観測されていないものを返す純粋メソッドを追加する
  - 既存メソッドのシグネチャは変更しない
  - 観測差分の正当性（結果は self の部分集合、各 dot は相手に未観測）を確認する sibling テストが green になる
  - _Requirements:_ 1.3, 1.4, 5.3
  - _Boundary:_ VersionVector
  - _Depends:_ none

- [x] 1.2 (P) LWWRegister に退役ノード整理を実装する
  - 値の書き込みノードが退役ノードのとき collapse 先へ置換する退役ノード整理を実装する（失敗不能）
  - 退役ノードの置換と非退役ノードの不変を確認する sibling テストが green になる
  - _Requirements:_ 5.3
  - _Boundary:_ LWWRegister
  - _Depends:_ none

- [x] 2. コア
- [x] 2.1 観測除去集合 ORSet のコアを実装する
  - 要素追加（自ノード識別を伴う）・削除・含有判定・列挙・全消去と、add-wins の状態ベース併合を実装する
  - 自身の mod 宣言と最小公開を配線し、key 型エイリアス `ORSetKey` を追加する
  - 並行 add/remove で add が残り、削除後の再追加が生存し、併合が順序非依存で収束する単体テストが green になる
  - _Requirements:_ 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 5.1, 6.1
  - _Boundary:_ ORSet
  - _Depends:_ 1.1

- [x] 2.2 ORSet の差分伝播と退役ノード整理を実装する
  - 自己参照デルタによる差分伝播、退役ノード整理（失敗不能）、因果配送前提マーカーを実装する
  - 差分適用が全状態併合と一致し、退役ノード整理が退役ノード以外の観測結果を保存する単体テストが green になる
  - _Requirements:_ 5.2, 5.3, 5.4
  - _Boundary:_ ORSet
  - _Depends:_ 2.1

- [x] 2.3 観測除去マップ ORMap のコアを実装する
  - キー集合を観測除去（ORSet 内包）で追跡し、置換 put と merge 更新 update を分離し、削除・取得・列挙・キー存在判定を実装する
  - 同一キーの値は CRDT として再帰併合する
  - 値に観測除去集合を put で差し替えてはならない契約を rustdoc に明示する
  - 自身の mod 宣言と最小公開を配線し、key 型エイリアス `ORMapKey` を追加する
  - 同一キー並行 update が値併合で収束し、削除と並行 update が競合してもキーが存続する単体テストが green になる
  - _Requirements:_ 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 5.1, 6.1
  - _Boundary:_ ORMap
  - _Depends:_ 2.2

- [x] 2.4 ORMap の差分伝播と条件付き退役ノード整理を実装する
  - 自己参照デルタによる差分伝播、因果配送前提マーカー、値型が退役ノード整理を実装する場合に整理を伝播する条件付き実装を行う
  - 差分適用が全状態併合と一致し、値型整理が伝播される単体テストが green になる
  - _Requirements:_ 5.2, 5.3, 5.4
  - _Boundary:_ ORMap
  - _Depends:_ 2.3

- [x] 2.5 多値マップ ORMultiMap を実装する
  - ORMap にキーごとの観測除去集合を載せた多値マップを実装する（バインディング追加・削除、取得、列挙、空集合になったキーは可視除去）
  - 内部 ORMap への委譲で併合・差分・退役ノード整理を満たす
  - 自身の mod 宣言と最小公開を配線し、key 型エイリアス `ORMultiMapKey` を追加する（配線ファイル ddata.rs / key.rs を 2.6 と共有するため逐次実行）
  - バインディング削除で空集合になったキーが不可視になり、同一キー同一要素の並行 add/remove で add が残る単体テストが green になる
  - _Requirements:_ 3.1, 3.2, 3.3, 3.4, 3.5, 5.1, 5.2, 5.3, 5.4, 6.1
  - _Boundary:_ ORMultiMap
  - _Depends:_ 2.4

- [x] 2.6 キー単位 LWW マップ LWWMap を実装する
  - ORMap にキーごとの LWWRegister を載せたマップを実装する（タイムスタンプ指定 put と clock 透過 put、削除、取得、列挙）
  - 内部 ORMap への委譲で併合・差分・退役ノード整理を満たす
  - 自身の mod 宣言と最小公開を配線し、key 型エイリアス `LWWMapKey` を追加する（配線ファイル ddata.rs / key.rs を 2.5 と共有するため逐次実行）
  - 同一キー並行 put で大きいタイムスタンプが勝ち、同値タイムスタンプで UniqueAddress 順の tie-break が効く単体テストが green になる
  - _Requirements:_ 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 5.1, 5.2, 5.3, 5.4, 6.1
  - _Boundary:_ LWWMap
  - _Depends:_ 2.4, 1.2

- [x] 3. 検証
- [x] 3.1 4 型の CRDT 則を property test で検証する
  - 各型について併合の可換律・結合律・冪等律を property test で検証する
  - 各型について差分適用が全状態併合と一致すること、退役ノード整理が退役ノード以外の観測結果を保存することを property test で検証する
  - 既存の property test 依存を再利用し、全 property test が green になる
  - _Requirements:_ 5.2, 5.3, 6.3
  - _Boundary:_ ddata CRDT テスト
  - _Depends:_ 2.2, 2.4, 2.5, 2.6

- [x] 3.2 no_std ビルドと構造 lint・既存 SPI 再利用を確認する
  - no_std ビルドと構造 lint（type-per-file / mod-file / tests-location / use-placement / redundant-fqcn / rustdoc / module-wiring）が通過することを確認する
  - 既存基底 SPI / VersionVector / LWWRegister を再利用し、同等の基盤型を新設していないことを確認する
  - 対象 crate の targeted check（cargo test -p、no-std、clippy、対象 dylint）が exit 0 で通過する
  - _Requirements:_ 6.2, 6.4
  - _Boundary:_ 全体検証
  - _Depends:_ 3.1
