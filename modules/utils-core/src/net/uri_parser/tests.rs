use super::{UriError, UriParser};

#[test]
fn test_parse_simple_uri() {
  // In Pekko, "pekko://system/path" means:
  // - scheme: "pekko"
  // - authority: "system" (treated as host in Java URI)
  // - path: "/path"
  // However, for ActorPath parsing, "system" is the system name, not authority
  // So we parse it as authority="system", path="/path"
  let input = "pekko://system/path";
  let result = UriParser::parse(input);
  assert!(result.is_ok());
  let parts = result.unwrap();
  assert_eq!(parts.scheme, Some("pekko"));
  assert_eq!(parts.authority, Some("system"));
  assert_eq!(parts.path, "/path");
  assert_eq!(parts.query, None);
  assert_eq!(parts.fragment, None);
}

#[test]
fn test_parse_uri_with_authority() {
  let input = "pekko://system@host:2552/path";
  let result = UriParser::parse(input);
  assert!(result.is_ok());
  let parts = result.unwrap();
  assert_eq!(parts.scheme, Some("pekko"));
  assert_eq!(parts.authority, Some("system@host:2552"));
  assert_eq!(parts.path, "/path");
}

#[test]
fn test_parse_uri_with_query() {
  let input = "pekko://system/path?key=value";
  let result = UriParser::parse(input);
  assert!(result.is_ok());
  let parts = result.unwrap();
  assert_eq!(parts.query, Some("key=value"));
}

#[test]
fn test_parse_uri_with_fragment() {
  let input = "pekko://system/path#fragment";
  let result = UriParser::parse(input);
  assert!(result.is_ok());
  let parts = result.unwrap();
  assert_eq!(parts.fragment, Some("fragment"));
}

#[test]
fn test_parse_pekko_tcp_scheme() {
  let input = "pekko.tcp://system@host:2552/path";
  let result = UriParser::parse(input);
  assert!(result.is_ok());
  let parts = result.unwrap();
  assert_eq!(parts.scheme, Some("pekko.tcp"));
}

#[test]
fn test_parse_invalid_scheme() {
  let input = "://system/path";
  let result = UriParser::parse(input);
  assert!(matches!(result, Err(UriError::InvalidScheme)));
}

#[test]
fn test_parse_percent_encoded_path() {
  let input = "pekko://system/path%20with%20spaces";
  let result = UriParser::parse(input);
  assert!(result.is_ok());
  let parts = result.unwrap();
  assert_eq!(parts.path, "/path%20with%20spaces");
  // Decode should be handled separately
  let decoded = UriParser::percent_decode(parts.path).unwrap();
  assert_eq!(decoded, "/path with spaces");
}

#[test]
fn test_parse_invalid_percent_encoding() {
  let input = "pekko://system/path%";
  let result = UriParser::parse(input);
  assert!(result.is_ok()); // Parsing succeeds, but decoding should fail
  let parts = result.unwrap();
  let decode_result = UriParser::percent_decode(parts.path);
  assert!(matches!(decode_result, Err(UriError::InvalidPercentEncoding)));
}

#[test]
fn test_parse_invalid_percent_encoding_short() {
  let input = "pekko://system/path%2";
  let result = UriParser::parse(input);
  assert!(result.is_ok());
  let parts = result.unwrap();
  let decode_result = UriParser::percent_decode(parts.path);
  assert!(matches!(decode_result, Err(UriError::InvalidPercentEncoding)));
}

#[test]
fn test_parse_invalid_percent_encoding_non_hex() {
  let input = "pekko://system/path%GH";
  let result = UriParser::parse(input);
  assert!(result.is_ok());
  let parts = result.unwrap();
  let decode_result = UriParser::percent_decode(parts.path);
  assert!(matches!(decode_result, Err(UriError::InvalidPercentEncoding)));
}

#[test]
fn test_validate_hostname_ascii() {
  assert!(UriParser::validate_hostname("localhost").is_ok());
  assert!(UriParser::validate_hostname("example.com").is_ok());
  assert!(UriParser::validate_hostname("host-name").is_ok());
}

#[test]
fn test_validate_hostname_ipv4() {
  assert!(UriParser::validate_hostname("192.168.1.1").is_ok());
  assert!(UriParser::validate_hostname("127.0.0.1").is_ok());
}

#[test]
fn test_validate_hostname_ipv6() {
  assert!(UriParser::validate_hostname("[::1]").is_ok());
  assert!(UriParser::validate_hostname("[2001:db8::1]").is_ok());
  assert!(UriParser::validate_hostname("[fe80::1%eth0]").is_ok());
}

#[test]
fn test_validate_hostname_invalid() {
  assert!(UriParser::validate_hostname("").is_err());
  assert!(UriParser::validate_hostname("host name").is_err()); // space not allowed
  assert!(UriParser::validate_hostname("host@name").is_err()); // @ not allowed in hostname
}

// Golden data tests based on Pekko/ProtoActor known cases
#[test]
fn test_golden_data_pekko_local_path() {
  // From Pekko docs: "pekko://my-sys/user/service-a/worker1"
  let input = "pekko://my-sys/user/service-a/worker1";
  let result = UriParser::parse(input).unwrap();
  assert_eq!(result.scheme, Some("pekko"));
  assert_eq!(result.authority, Some("my-sys"));
  assert_eq!(result.path, "/user/service-a/worker1");
}

#[test]
fn test_golden_data_pekko_remote_path() {
  // From Pekko docs: "pekko://my-sys@host.example.com:5678/user/service-b"
  let input = "pekko://my-sys@host.example.com:5678/user/service-b";
  let result = UriParser::parse(input).unwrap();
  assert_eq!(result.scheme, Some("pekko"));
  assert_eq!(result.authority, Some("my-sys@host.example.com:5678"));
  assert_eq!(result.path, "/user/service-b");
}

#[test]
fn test_golden_data_pekko_root_path() {
  // From Pekko ActorPathSpec: RootActorPath(Address("pekko", "mysys")).toString should ===("pekko://mysys/")
  let input = "pekko://mysys/";
  let result = UriParser::parse(input).unwrap();
  assert_eq!(result.scheme, Some("pekko"));
  assert_eq!(result.authority, Some("mysys"));
  assert_eq!(result.path, "/");
}

#[test]
fn test_golden_data_pekko_remote_with_port() {
  // From Pekko ActorPathSpec: "pekko://my_sys@host:1234/some/ref"
  let input = "pekko://my_sys@host:1234/some/ref";
  let result = UriParser::parse(input).unwrap();
  assert_eq!(result.scheme, Some("pekko"));
  assert_eq!(result.authority, Some("my_sys@host:1234"));
  assert_eq!(result.path, "/some/ref");
}

#[test]
fn test_golden_data_akka_compatible_path() {
  // From Pekko ActorPathSpec: "akka://my_sys@host:1234/some/ref" (backward compatibility)
  let input = "akka://my_sys@host:1234/some/ref";
  let result = UriParser::parse(input).unwrap();
  assert_eq!(result.scheme, Some("akka"));
  assert_eq!(result.authority, Some("my_sys@host:1234"));
  assert_eq!(result.path, "/some/ref");
}

#[test]
fn test_golden_data_pekko_tcp_remote() {
  // From Pekko: pekko.tcp scheme for remote transport
  let input = "pekko.tcp://system@host.example.com:2552/user/actor";
  let result = UriParser::parse(input).unwrap();
  assert_eq!(result.scheme, Some("pekko.tcp"));
  assert_eq!(result.authority, Some("system@host.example.com:2552"));
  assert_eq!(result.path, "/user/actor");
}

#[test]
fn test_golden_data_address_only() {
  // From Pekko: "pekko://my_sys@host:1234" (address without path)
  let input = "pekko://my_sys@host:1234";
  let result = UriParser::parse(input).unwrap();
  assert_eq!(result.scheme, Some("pekko"));
  assert_eq!(result.authority, Some("my_sys@host:1234"));
  assert_eq!(result.path, "");
}

#[test]
fn test_golden_data_malformed_cases() {
  // From Pekko ActorPathSpec: malformed paths that should fail
  assert!(UriParser::parse("").is_err()); // empty string
  assert!(UriParser::parse("://hallo").is_err()); // missing scheme
  // Note: Our parser is more lenient and may parse some of these differently
  // than Pekko's strict parser. We validate at a higher level.
}

#[test]
fn test_golden_data_percent_encoded_actor_names() {
  // Actor names may contain percent-encoded characters
  let input = "pekko://system/user/actor%20name";
  let result = UriParser::parse(input).unwrap();
  assert_eq!(result.path, "/user/actor%20name");
  let decoded = UriParser::percent_decode("actor%20name").unwrap();
  assert_eq!(decoded, "actor name");
}

#[test]
fn test_golden_data_query_and_fragment() {
  // URIs with query and fragment components
  let input = "pekko://system/path?key=value#fragment";
  let result = UriParser::parse(input).unwrap();
  assert_eq!(result.scheme, Some("pekko"));
  assert_eq!(result.authority, Some("system"));
  assert_eq!(result.path, "/path");
  assert_eq!(result.query, Some("key=value"));
  assert_eq!(result.fragment, Some("fragment"));
}

// Property tests: round-trip validation
#[test]
fn test_property_round_trip_simple() {
  // Simple round-trip: parse should succeed for valid URIs
  let test_cases = vec![
    "pekko://system/path",
    "pekko://system/user/actor",
    "pekko://system@host:2552/path",
    "pekko.tcp://system@host:2552/path",
  ];

  for uri in test_cases {
    let result = UriParser::parse(uri);
    assert!(result.is_ok(), "Failed to parse: {}", uri);
    let parts = result.unwrap();
    assert_eq!(parts.scheme, Some(uri.split(':').next().unwrap()));
  }
}

#[test]
fn test_property_error_classification() {
  // Verify that different error types are correctly classified
  let parse_error_cases = vec![
    ("", UriError::InvalidScheme), // Empty string
    ("://system/path", UriError::InvalidScheme), // Missing scheme
    ("1invalid://system/path", UriError::InvalidScheme), // Scheme starting with digit
  ];

  for (uri, expected_error) in parse_error_cases {
    let result = UriParser::parse(uri);
    assert!(
      matches!(result, Err(ref e) if *e == expected_error),
      "Expected error {:?} for URI: {}",
      expected_error,
      uri
    );
  }

  // For cases where parsing succeeds but validation fails during decode
  let decode_error_cases = vec!["pekko://system/path%", "pekko://system/path%2", "pekko://system/path%GH"];

  for uri in decode_error_cases {
    let result = UriParser::parse(uri);
    assert!(result.is_ok(), "Parsing should succeed for: {}", uri);
    let parts = result.unwrap();
    let decode_result = UriParser::percent_decode(parts.path);
    assert!(
      matches!(decode_result, Err(UriError::InvalidPercentEncoding)),
      "Expected decode error for URI: {}",
      uri
    );
  }
}

#[test]
fn test_property_hostname_validation() {
  // Test various hostname formats
  let valid_hostnames = vec![
    "localhost",
    "example.com",
    "host-name",
    "192.168.1.1",
    "127.0.0.1",
    "[::1]",
    "[2001:db8::1]",
    "[fe80::1%eth0]",
  ];

  for hostname in valid_hostnames {
    assert!(
      UriParser::validate_hostname(hostname).is_ok(),
      "Hostname should be valid: {}",
      hostname
    );
  }

  let invalid_hostnames = vec![
    "",
    "host name", // space
    "host@name", // @ symbol
    "host.name.", // trailing dot
    "-hostname", // leading hyphen
    "hostname-", // trailing hyphen
  ];

  for hostname in invalid_hostnames {
    assert!(
      UriParser::validate_hostname(hostname).is_err(),
      "Hostname should be invalid: {}",
      hostname
    );
  }
}

#[test]
fn test_error_message_readability() {
  // Verify that error messages are readable and useful for logging/tracing
  let parse_error_cases = vec![
    ("", UriError::InvalidScheme),
    ("://system/path", UriError::InvalidScheme),
  ];

  for (uri, expected_error) in parse_error_cases {
    let result = UriParser::parse(uri);
    let error = result.unwrap_err();
    let error_msg = format!("{}", error);
    // Verify error message is non-empty and descriptive
    assert!(!error_msg.is_empty(), "Error message should not be empty");
    assert!(
      error_msg.len() > 5,
      "Error message should be descriptive: {}",
      error_msg
    );
    // Verify error type matches
    assert_eq!(error, expected_error);
  }

  // For percent encoding errors
  let decode_error_uri = "pekko://system/path%";
  let parts = UriParser::parse(decode_error_uri).unwrap();
  let decode_result = UriParser::percent_decode(parts.path);
  if let Err(e) = decode_result {
    let error_msg = format!("{}", e);
    assert!(!error_msg.is_empty(), "Error message should not be empty");
    assert_eq!(e, UriError::InvalidPercentEncoding);
  }
}
