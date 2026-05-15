use super::parse_remote_authority;

#[test]
fn parse_remote_authority_accepts_ipv4_host() {
  let address = parse_remote_authority("system@127.0.0.1:2552").unwrap();

  assert_eq!(address.system(), "system");
  assert_eq!(address.host(), "127.0.0.1");
  assert_eq!(address.port(), 2552);
}

#[test]
fn parse_remote_authority_unwraps_bracketed_ipv6_host() {
  let address = parse_remote_authority("system@[2001:db8::1]:2552").unwrap();

  assert_eq!(address.system(), "system");
  assert_eq!(address.host(), "2001:db8::1");
  assert_eq!(address.port(), 2552);
}

#[test]
fn parse_remote_authority_rejects_invalid_port() {
  assert_eq!(parse_remote_authority("system@127.0.0.1:not-a-port"), None);
}
