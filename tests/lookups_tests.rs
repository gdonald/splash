use splash::lookups::{reverse_dns, system_user_exists, LookupError, Lookups, KEYS};
use splash::output::{ParsedLine, Token, TokenKind};
use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn example_path(filename: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("examples")
        .join(filename)
        .display()
        .to_string()
}

fn table(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Lookups turned on, with only the given lookups running
fn lookups(entries: &[(&str, &str)]) -> Lookups {
    let mut configured = table(&[("users", "false"), ("ports", "false")]);
    configured.extend(table(entries));

    Lookups::build(Some(true), &configured).unwrap()
}

fn listed_users() -> Lookups {
    lookups(&[
        ("users", "true"),
        ("passwd", &example_path("lookups_passwd.txt")),
    ])
}

fn listed_services() -> Lookups {
    lookups(&[
        ("ports", "true"),
        ("services", &example_path("lookups_services.txt")),
    ])
}

fn listed_networks() -> Lookups {
    lookups(&[("geoip", &example_path("lookups_networks.csv"))])
}

fn pieces(parsed: &ParsedLine) -> Vec<(String, TokenKind)> {
    parsed
        .tokens
        .iter()
        .map(|token| (token.text.to_string(), token.kind))
        .collect()
}

fn piece(text: &str, kind: TokenKind) -> (String, TokenKind) {
    (text.to_string(), kind)
}

fn message(text: &str) -> ParsedLine<'_> {
    ParsedLine::new(vec![Token::new(text, TokenKind::Message)])
}

fn address(text: &str) -> ParsedLine<'_> {
    ParsedLine::new(vec![Token::new(text, TokenKind::Ip)])
}

fn gateway(address: Ipv4Addr) -> Option<String> {
    (address == Ipv4Addr::new(10, 0, 0, 5)).then(|| "gateway.lan".to_string())
}

static COUNTED_LOOKUPS: AtomicUsize = AtomicUsize::new(0);

fn counted_gateway(address: Ipv4Addr) -> Option<String> {
    COUNTED_LOOKUPS.fetch_add(1, Ordering::SeqCst);
    gateway(address)
}

#[test]
fn no_lookups_leave_a_line_as_it_was() {
    let line = ParsedLine::new(vec![
        Token::new("nobody-here", TokenKind::UserId),
        Token::new(" port 22", TokenKind::Message),
    ]);

    assert_eq!(Lookups::none().apply(line.clone()), line);
}

#[test]
fn the_default_lookups_look_nothing_up() {
    assert_eq!(Lookups::default(), Lookups::none());
}

#[test]
fn lookups_are_off_unless_turned_on() {
    assert_eq!(
        Lookups::build(None, &BTreeMap::new()).unwrap(),
        Lookups::none()
    );
}

#[test]
fn the_config_file_can_turn_lookups_on() {
    let configured = table(&[("enabled", "true"), ("ports", "false")]);

    assert_ne!(Lookups::build(None, &configured).unwrap(), Lookups::none());
}

#[test]
fn turning_lookups_off_wins_over_the_config_file() {
    let configured = table(&[("enabled", "true")]);

    assert_eq!(
        Lookups::build(Some(false), &configured).unwrap(),
        Lookups::none()
    );
}

#[test]
fn turning_lookups_on_wins_over_the_config_file() {
    let configured = table(&[("enabled", "false"), ("ports", "false")]);

    assert_ne!(
        Lookups::build(Some(true), &configured).unwrap(),
        Lookups::none()
    );
}

#[test]
fn the_lookups_table_keys_are_listed() {
    assert_eq!(
        KEYS,
        ["enabled", "users", "ports", "dns", "passwd", "services", "geoip"]
    );
}

#[test]
fn an_unknown_lookups_key_is_rejected() {
    assert_eq!(
        Lookups::build(None, &table(&[("whois", "true")])),
        Err(LookupError::UnknownKey("whois".to_string()))
    );
}

#[test]
fn a_switch_that_is_not_true_or_false_is_rejected_even_with_lookups_off() {
    assert_eq!(
        Lookups::build(None, &table(&[("dns", "yes")])),
        Err(LookupError::BadSwitch {
            key: "dns".to_string(),
            value: "yes".to_string()
        })
    );
}

#[test]
fn lookup_errors_describe_the_problem() {
    let messages: Vec<String> = [
        LookupError::UnknownKey("whois".to_string()),
        LookupError::BadSwitch {
            key: "dns".to_string(),
            value: "yes".to_string(),
        },
        LookupError::Io {
            path: "/missing".to_string(),
            message: "No such file or directory".to_string(),
        },
        LookupError::BadNetwork {
            path: "networks.csv".to_string(),
            line: 3,
        },
    ]
    .iter()
    .map(|error| error.to_string())
    .collect();

    assert_eq!(
        messages,
        vec![
            "Unknown lookups key 'whois'",
            "Invalid lookups setting dns = 'yes' (expected true or false)",
            "Could not read /missing: No such file or directory",
            "networks.csv: line 3: expected NETWORK/BITS,COUNTRY,CITY",
        ]
    );
}

#[test]
fn a_user_the_passwd_file_lists_keeps_the_user_style() {
    let line = ParsedLine::new(vec![Token::new("alice", TokenKind::UserId)]);

    assert_eq!(
        pieces(&listed_users().apply(line)),
        vec![piece("alice", TokenKind::UserId)]
    );
}

#[test]
fn a_user_the_passwd_file_does_not_list_is_marked_unknown() {
    let line = ParsedLine::new(vec![Token::new("admin", TokenKind::UserId)]);

    assert_eq!(
        pieces(&listed_users().apply(line)),
        vec![piece("admin", TokenKind::UnknownUser)]
    );
}

#[test]
fn a_dash_in_place_of_a_user_is_left_alone() {
    let line = ParsedLine::new(vec![Token::new("-", TokenKind::UserId)]);

    assert_eq!(
        pieces(&listed_users().apply(line)),
        vec![piece("-", TokenKind::UserId)]
    );
}

#[test]
fn only_user_fields_are_checked_against_the_passwd_file() {
    let line = ParsedLine::new(vec![Token::new("admin", TokenKind::Message)]);

    assert_eq!(
        pieces(&listed_users().apply(line)),
        vec![piece("admin", TokenKind::Message)]
    );
}

#[test]
fn user_lookups_can_be_turned_off() {
    let line = ParsedLine::new(vec![Token::new("admin", TokenKind::UserId)]);
    let lookups = lookups(&[
        ("passwd", &example_path("lookups_passwd.txt")),
        ("geoip", &example_path("lookups_networks.csv")),
    ]);

    assert_eq!(
        pieces(&lookups.apply(line)),
        vec![piece("admin", TokenKind::UserId)]
    );
}

#[test]
fn a_missing_passwd_file_is_reported() {
    let result = Lookups::build(Some(true), &table(&[("passwd", "/no/such/passwd")]));

    assert!(matches!(
        result,
        Err(LookupError::Io { path, .. }) if path == "/no/such/passwd"
    ));
}

#[test]
fn users_are_checked_against_the_system_accounts_without_a_passwd_file() {
    let line = ParsedLine::new(vec![
        Token::new("root", TokenKind::UserId),
        Token::new(" ", TokenKind::Plain),
        Token::new("no-such-splash-user", TokenKind::UserId),
        Token::new(" ", TokenKind::Plain),
        Token::new("root", TokenKind::UserId),
    ]);

    assert_eq!(
        pieces(&lookups(&[("users", "true")]).apply(line)),
        vec![
            piece("root", TokenKind::UserId),
            piece(" ", TokenKind::Plain),
            piece("no-such-splash-user", TokenKind::UnknownUser),
            piece(" ", TokenKind::Plain),
            piece("root", TokenKind::UserId),
        ]
    );
}

#[test]
fn the_system_accounts_hold_root() {
    assert!(system_user_exists("root"));
}

#[test]
fn the_system_accounts_do_not_hold_a_made_up_user() {
    assert!(!system_user_exists("no-such-splash-user"));
}

#[test]
fn a_user_name_holding_a_nul_byte_is_not_a_system_account() {
    assert!(!system_user_exists("ro\0ot"));
}

#[test]
fn a_port_is_followed_by_its_service() {
    assert_eq!(
        pieces(&listed_services().apply(message("from 10.0.0.5 port 22 ssh2"))),
        vec![
            piece("from 10.0.0.5 port 22", TokenKind::Message),
            piece("(ssh)", TokenKind::Service),
            piece(" ssh2", TokenKind::Message),
        ]
    );
}

#[test]
fn a_port_split_across_tokens_is_followed_by_its_service() {
    let line = ParsedLine::new(vec![
        Token::new("Port", TokenKind::Message),
        Token::new(" ", TokenKind::Plain),
        Token::new("443", TokenKind::Number),
    ]);

    assert_eq!(
        pieces(&listed_services().apply(line)),
        vec![
            piece("Port", TokenKind::Message),
            piece(" ", TokenKind::Plain),
            piece("443", TokenKind::Number),
            piece("(https)", TokenKind::Service),
        ]
    );
}

#[test]
fn a_port_after_an_address_is_followed_by_its_service() {
    assert_eq!(
        pieces(&listed_services().apply(address("10.0.0.5:8080"))),
        vec![
            piece("10.0.0.5:8080", TokenKind::Ip),
            piece("(http-alt)", TokenKind::Service),
        ]
    );
}

#[test]
fn a_port_with_no_known_service_is_left_alone() {
    assert_eq!(
        pieces(&listed_services().apply(message("port 40022"))),
        vec![piece("port 40022", TokenKind::Message)]
    );
}

#[test]
fn a_number_too_large_to_be_a_port_is_left_alone() {
    assert_eq!(
        pieces(&listed_services().apply(message("port 99999"))),
        vec![piece("port 99999", TokenKind::Message)]
    );
}

#[test]
fn ports_are_named_from_the_system_services_file_by_default() {
    assert_eq!(
        pieces(&lookups(&[("ports", "true")]).apply(message("port 22"))),
        vec![
            piece("port 22", TokenKind::Message),
            piece("(ssh)", TokenKind::Service),
        ]
    );
}

#[test]
fn a_missing_services_file_is_reported() {
    let result = Lookups::build(Some(true), &table(&[("services", "/no/such/services")]));

    assert!(matches!(
        result,
        Err(LookupError::Io { path, .. }) if path == "/no/such/services"
    ));
}

#[test]
fn reverse_dns_names_the_host_of_an_address() {
    let lookups = lookups(&[]).with_resolver(gateway);

    assert_eq!(
        pieces(&lookups.apply(address("10.0.0.5"))),
        vec![
            piece("10.0.0.5", TokenKind::Ip),
            piece(" (gateway.lan)", TokenKind::Host),
        ]
    );
}

#[test]
fn an_address_with_no_host_name_is_left_alone() {
    let lookups = lookups(&[]).with_resolver(gateway);

    assert_eq!(
        pieces(&lookups.apply(address("10.0.0.6"))),
        vec![piece("10.0.0.6", TokenKind::Ip)]
    );
}

#[test]
fn an_access_log_client_is_followed_by_its_host_name() {
    let line = ParsedLine::new(vec![
        Token::new("10.0.0.5", TokenKind::Client),
        Token::new(" - frank", TokenKind::Plain),
    ]);
    let lookups = lookups(&[]).with_resolver(gateway);

    assert_eq!(
        pieces(&lookups.apply(line)),
        vec![
            piece("10.0.0.5", TokenKind::Client),
            piece(" (gateway.lan)", TokenKind::Host),
            piece(" - frank", TokenKind::Plain),
        ]
    );
}

#[test]
fn an_address_inside_other_text_is_followed_by_its_host_name() {
    let lookups = lookups(&[]).with_resolver(gateway);

    assert_eq!(
        pieces(&lookups.apply(address("from=10.0.0.5,"))),
        vec![
            piece("from=10.0.0.5", TokenKind::Ip),
            piece(" (gateway.lan)", TokenKind::Host),
            piece(",", TokenKind::Ip),
        ]
    );
}

#[test]
fn text_that_is_not_a_valid_address_is_not_looked_up() {
    let lookups = lookups(&[]).with_resolver(gateway);

    assert_eq!(
        pieces(&lookups.apply(address("999.0.0.5"))),
        vec![piece("999.0.0.5", TokenKind::Ip)]
    );
}

#[test]
fn each_address_is_resolved_once() {
    let lookups = lookups(&[]).with_resolver(counted_gateway);

    lookups.apply(address("10.0.0.5"));
    lookups.apply(address("10.0.0.5"));

    assert_eq!(COUNTED_LOOKUPS.load(Ordering::SeqCst), 1);
}

#[test]
fn the_config_file_turns_reverse_dns_on() {
    assert_ne!(lookups(&[("dns", "true")]), lookups(&[]));
}

#[test]
fn reverse_dns_reads_the_hosts_file() {
    assert_eq!(
        reverse_dns(Ipv4Addr::LOCALHOST),
        Some("localhost".to_string())
    );
}

#[test]
fn an_address_is_followed_by_its_location() {
    assert_eq!(
        pieces(&listed_networks().apply(address("203.0.113.7"))),
        vec![
            piece("203.0.113.7", TokenKind::Ip),
            piece(" (US, Boston)", TokenKind::Location),
        ]
    );
}

#[test]
fn a_network_with_no_city_gives_the_country_alone() {
    assert_eq!(
        pieces(&listed_networks().apply(address("198.51.100.20"))),
        vec![
            piece("198.51.100.20", TokenKind::Ip),
            piece(" (DE)", TokenKind::Location),
        ]
    );
}

#[test]
fn an_address_outside_every_network_has_no_location() {
    assert_eq!(
        pieces(&listed_networks().apply(address("192.0.2.1"))),
        vec![piece("192.0.2.1", TokenKind::Ip)]
    );
}

#[test]
fn the_narrowest_network_holding_an_address_gives_its_location() {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("networks.csv");
    std::fs::write(
        &path,
        "0.0.0.0/0,ZZ\n10.1.0.0/16,US,Denver\n10.0.0.0/8,US\n",
    )
    .unwrap();
    let lookups = lookups(&[("geoip", &path.display().to_string())]);

    assert_eq!(
        pieces(&lookups.apply(address("10.1.2.3"))),
        vec![
            piece("10.1.2.3", TokenKind::Ip),
            piece(" (US, Denver)", TokenKind::Location),
        ]
    );
}

#[test]
fn a_bad_network_line_is_reported_with_its_line_number() {
    for line in [
        "203.0.113.0,US",
        "203.0.113/24,US",
        "203.0.113.0/33,US",
        "203.0.113.0/24",
    ] {
        let directory = tempfile::TempDir::new().unwrap();
        let path = directory.path().join("networks.csv");
        std::fs::write(&path, format!("# networks\n\n{}\n", line)).unwrap();
        let path = path.display().to_string();

        assert_eq!(
            Lookups::build(Some(true), &table(&[("ports", "false"), ("geoip", &path)])),
            Err(LookupError::BadNetwork { path, line: 3 }),
            "{}",
            line
        );
    }
}

#[test]
fn a_port_after_an_address_comes_before_its_host_name_and_location() {
    let lookups = lookups(&[
        ("ports", "true"),
        ("services", &example_path("lookups_services.txt")),
        ("geoip", &example_path("lookups_networks.csv")),
    ])
    .with_resolver(|_| Some("edge.example.com".to_string()));

    assert_eq!(
        pieces(&lookups.apply(address("203.0.113.7:443"))),
        vec![
            piece("203.0.113.7:443", TokenKind::Ip),
            piece("(https)", TokenKind::Service),
            piece(" (edge.example.com)", TokenKind::Host),
            piece(" (US, Boston)", TokenKind::Location),
        ]
    );
}
