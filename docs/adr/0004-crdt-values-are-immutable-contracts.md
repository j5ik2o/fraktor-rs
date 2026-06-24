# CRDT value は immutable contract として扱う

Distributed Data (分散データ) の CRDT (収束データ型) は immutable value contract として扱い、merge や update は既存 instance をその場で変更せず新しい value を返す。これにより `no_std` 制約下でも merge law test、delta comparison、pruning behavior を明示できるため、並行 version や property test の推論を難しくする `&mut self` API や consuming `self` API は採用しない。

**Considered Options**

- `&mut self` で既存 value を更新する案: 更新前後の version と並行 replica の比較が implicit になり、merge law test と pruning behavior の説明性を落とすため不採用。
- consuming `self` で更新後 value を返す案: 所有権の移動で単純な更新 API は作れるが、複数 replica version を同時に比較する property test が扱いにくくなるため不採用。
- immutable value として新しい value を返す案: `no_std` 下でも状態遷移を明示でき、delta comparison と pruning の検証を単純に保てるため採用。
