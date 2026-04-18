use crate::core::attributes::{Attributes, NestedMaterializationCancellationPolicy};

// --- 定数値テスト（Pekko parity） ---

#[test]
fn eager_cancellation_has_propagate_false() {
  // Given: Pekko `Attributes.EagerCancellation` 相当の定数
  let policy = NestedMaterializationCancellationPolicy::EAGER_CANCELLATION;

  // Then: propagate_to_nested_materialization は false
  assert!(!policy.propagate_to_nested_materialization());
}

#[test]
fn propagate_to_nested_has_propagate_true() {
  // Given: Pekko `Attributes.PropagateToNested` 相当の定数
  let policy = NestedMaterializationCancellationPolicy::PROPAGATE_TO_NESTED;

  // Then: propagate_to_nested_materialization は true
  assert!(policy.propagate_to_nested_materialization());
}

#[test]
fn default_equals_eager_cancellation() {
  // Given: Pekko の `Default = EagerCancellation` 規約
  let default_policy = NestedMaterializationCancellationPolicy::DEFAULT;

  // Then: DEFAULT は EAGER_CANCELLATION と等価
  assert_eq!(default_policy, NestedMaterializationCancellationPolicy::EAGER_CANCELLATION);
  assert!(!default_policy.propagate_to_nested_materialization());
}

// --- コンストラクタ / アクセサ ---

#[test]
fn new_constructs_with_given_flag() {
  // Given: 明示的な true を渡して構築
  let policy = NestedMaterializationCancellationPolicy::new(true);

  // Then: アクセサは同じ値を返し、定数と等価
  assert!(policy.propagate_to_nested_materialization());
  assert_eq!(policy, NestedMaterializationCancellationPolicy::PROPAGATE_TO_NESTED);
}

#[test]
fn new_with_false_equals_eager_cancellation() {
  // Given: false で構築
  let policy = NestedMaterializationCancellationPolicy::new(false);

  // Then: EAGER_CANCELLATION と等価
  assert_eq!(policy, NestedMaterializationCancellationPolicy::EAGER_CANCELLATION);
  assert!(!policy.propagate_to_nested_materialization());
}

// --- 等価性 / Clone / Copy ---

#[test]
fn same_values_are_equal() {
  let a = NestedMaterializationCancellationPolicy::EAGER_CANCELLATION;
  let b = NestedMaterializationCancellationPolicy::new(false);
  assert_eq!(a, b);
}

#[test]
fn different_values_are_not_equal() {
  assert_ne!(
    NestedMaterializationCancellationPolicy::EAGER_CANCELLATION,
    NestedMaterializationCancellationPolicy::PROPAGATE_TO_NESTED
  );
}

#[test]
fn clone_preserves_value() {
  let original = NestedMaterializationCancellationPolicy::PROPAGATE_TO_NESTED;
  let cloned = original.clone();
  assert_eq!(original, cloned);
}

#[test]
fn copy_semantics_work() {
  let lhs = NestedMaterializationCancellationPolicy::PROPAGATE_TO_NESTED;
  let rhs = lhs;
  assert_eq!(lhs, rhs);
}

// --- Debug フォーマット ---

#[test]
fn debug_format_is_non_empty() {
  let debug = alloc::format!("{:?}", NestedMaterializationCancellationPolicy::EAGER_CANCELLATION);
  assert!(!debug.is_empty());
}

// --- Attributes::mandatory_attribute<T: MandatoryAttribute> 経由取得 ---

#[test]
fn mandatory_attribute_retrieval_returns_stored_policy() {
  // Given: PROPAGATE_TO_NESTED を保持する Attributes コレクション
  let policy = NestedMaterializationCancellationPolicy::PROPAGATE_TO_NESTED;
  let attrs = Attributes::nested_materialization_cancellation_policy(policy);

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<NestedMaterializationCancellationPolicy>();

  // Then: 格納した値と等価なインスタンスが取り出せる
  let got = retrieved.expect("policy must be retrievable as mandatory attribute");
  assert_eq!(*got, policy);
}

#[test]
fn mandatory_attribute_returns_none_when_absent() {
  // Given: 当該 attribute を持たない空の Attributes
  let attrs = Attributes::new();

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<NestedMaterializationCancellationPolicy>();

  // Then: None が返る
  assert!(retrieved.is_none());
}
