use alloc::{string::String, vec};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use super::{age_ordered, member_age_order, oldest_member};
use crate::membership::{DataCenter, MembershipVersion, NodeRecord, NodeStatus};

fn make_record(authority: &str, join_v: u64) -> NodeRecord {
  NodeRecord::new(
    String::from("node"),
    String::from(authority),
    NodeStatus::Up,
    MembershipVersion::new(join_v),
    String::from("1.0.0"),
    vec![],
  )
}

// 要件 1.1, 1.2: 入力順をシャッフルしても同一の並びになる決定性
#[test]
fn age_ordered_is_deterministic_regardless_of_input_order() {
  let a = make_record("n1:4000", 1);
  let b = make_record("n2:4000", 2);
  let c = make_record("n3:4000", 3);

  let forward = vec![a.clone(), b.clone(), c.clone()];
  let reversed = vec![c.clone(), b.clone(), a.clone()];
  let shuffled = vec![b.clone(), a.clone(), c.clone()];

  let r1 = age_ordered(&forward);
  let r2 = age_ordered(&reversed);
  let r3 = age_ordered(&shuffled);

  assert_eq!(r1, r2, "reversed input must produce same order");
  assert_eq!(r1, r3, "shuffled input must produce same order");

  // 要件 1.3: 参加が古い順（join_version 昇順）
  assert_eq!(r1[0].join_version, MembershipVersion::new(1));
  assert_eq!(r1[1].join_version, MembershipVersion::new(2));
  assert_eq!(r1[2].join_version, MembershipVersion::new(3));
}

// 要件 1.4: join_version 同値のとき authority で tie-break が一意に解決される
#[test]
fn age_ordered_breaks_tie_by_authority() {
  use core::cmp::Ordering;

  // n1:999 は n1:10000 より小さいポートなので "older"（is_older_than と同一）
  let low_port = make_record("n1:999", 10);
  let high_port = make_record("n1:10000", 10);

  // member_age_order の全順序: low_port が先
  assert_eq!(member_age_order(&low_port, &high_port), Ordering::Less);
  assert_eq!(member_age_order(&high_port, &low_port), Ordering::Greater);
  assert_eq!(member_age_order(&low_port, &low_port), Ordering::Equal);

  let input = [high_port.clone(), low_port.clone()];
  let ordered = age_ordered(&input);
  assert_eq!(ordered[0].authority, "n1:999");
  assert_eq!(ordered[1].authority, "n1:10000");
}

// 要件 1.5: oldest_member は age_ordered の先頭と一致する
#[test]
fn oldest_member_matches_head_of_age_ordered() {
  let a = make_record("n1:4000", 1);
  let b = make_record("n2:4000", 2);
  let records = vec![b.clone(), a.clone()];

  let ordered = age_ordered(&records);
  let oldest = oldest_member(&records).expect("must be Some for non-empty slice");
  assert_eq!(oldest.authority, ordered[0].authority);
  assert_eq!(oldest.join_version, MembershipVersion::new(1));
}

// 要件 1.6: 空集合では oldest_member が None を返す
#[test]
fn oldest_member_returns_none_for_empty_slice() {
  assert!(oldest_member(&[]).is_none());
}

// 全順序の反対称性を追加確認
#[test]
fn member_age_order_is_antisymmetric() {
  use core::cmp::Ordering;

  let a = make_record("n1:4000", 5);
  let b = make_record("n2:4000", 5);

  let ab = member_age_order(&a, &b);
  let ba = member_age_order(&b, &a);

  // 全順序の対称性: cmp(a, b) = cmp(b, a).reverse()
  match ab {
    | Ordering::Less => assert_eq!(ba, Ordering::Greater),
    | Ordering::Greater => assert_eq!(ba, Ordering::Less),
    | Ordering::Equal => assert_eq!(ba, Ordering::Equal),
  }
}

// join_version と authority が同一の別 incarnation でも全順序が一意に決まる
#[test]
fn member_age_order_breaks_tie_by_incarnation() {
  use core::cmp::Ordering;

  let make_incarnation = |uid: u64| {
    NodeRecord::new_with_identity(
      UniqueAddress::new(Address::new("fraktor-cluster", "n1", 4000), uid),
      DataCenter::default(),
      String::from("node"),
      NodeStatus::Up,
      MembershipVersion::new(7),
      String::from("1.0.0"),
      vec![],
    )
  };
  let old_incarnation = make_incarnation(1);
  let new_incarnation = make_incarnation(2);

  assert_eq!(member_age_order(&old_incarnation, &new_incarnation), Ordering::Less);
  assert_eq!(member_age_order(&new_incarnation, &old_incarnation), Ordering::Greater);

  // 入力順に依存せず oldest が一意に決まる
  let forward = [old_incarnation.clone(), new_incarnation.clone()];
  let reversed = [new_incarnation, old_incarnation];
  assert_eq!(oldest_member(&forward).map(|r| r.unique_address.uid()), Some(1));
  assert_eq!(oldest_member(&reversed).map(|r| r.unique_address.uid()), Some(1));
}
