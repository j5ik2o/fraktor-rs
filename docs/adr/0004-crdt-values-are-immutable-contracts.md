# CRDT value は immutable contract として扱う

Distributed Data の CRDT は immutable value contract として扱い、merge や update は既存 instance をその場で変更せず新しい value を返す。これにより `no_std` 制約下でも merge law test、delta comparison、pruning behavior を明示できるため、並行 version や property test の推論を難しくする `&mut self` API や consuming `self` API は採用しない。
