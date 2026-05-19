#![feature(register_tool)]
#![register_tool(redundant_fqcn)]
#![warn(redundant_fqcn::redundant_fqcn)]

mod domain {
  #[derive(Clone, Copy)]
  pub struct UserAccount;

  impl UserAccount {
    pub fn new() -> Self {
      Self
    }
  }
}

mod infra {
  use crate::domain::UserAccount as DomainUserAccount;

  pub struct UserAccount(pub DomainUserAccount);
}

use domain::UserAccount;

fn build() -> infra::UserAccount {
  let ua = UserAccount::new();
  crate::infra::UserAccount(ua)
}

fn main() {
  let _ = build();
}
