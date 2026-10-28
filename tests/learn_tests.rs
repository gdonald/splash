use splash::external::TomlPlugin;
use splash::learn::{learn, LearnError};

fn patterns(file: &str) -> Vec<String> {
    file.lines()
        .skip_while(|line| *line != "[patterns]")
        .skip(1)
        .map(str::to_string)
        .collect()
}

fn fields(file: &str, line: &str) -> Vec<(String, String)> {
    let plugin = TomlPlugin::parse(file).unwrap();
    let (_, parsed) = plugin.read(line).unwrap();

    parsed
        .tokens
        .iter()
        .filter(|token| token.kind.name() != "plain")
        .map(|token| (token.kind.name().to_string(), token.text.to_string()))
        .collect()
}

fn pair(key: &str, text: &str) -> (String, String) {
    (key.to_string(), text.to_string())
}

const ORDERS: &str = "\
2024-03-05 10:15:01,120 INFO [checkout] order 1001 placed in 120ms
2024-03-05 10:15:02,048 WARN [checkout] order 1002 retried in 3.5s

2024-03-05 10:15:04,511 ERROR [checkout] payment declined for order 1003
worker-2 GET /orders/1004 HTTP/1.1 200
worker-3 POST /orders HTTP/1.1 201
";

#[test]
fn test_learning_writes_a_plugin_named_for_the_format_with_a_pattern_for_each_line_form() {
    assert_eq!(
        learn("orders", ORDERS).unwrap(),
        concat!(
            "name = 'orders'\n",
            "description = 'Learned from 5 sample lines'\n",
            "\n",
            "[patterns]\n",
            r"line1 = '^(?P<timestamp>\d+\-\d+\-\d+\s+\d+:\d+:\d+,\d+)\s+(?P<level>[A-Za-z]+)\s+\[checkout\]\s+(?P<message>.*)$'",
            "\n",
            r#"line2 = '^[A-Za-z]+\-\d+\s+(?P<method>[A-Z]+)\s+(?P<path>[^\s\[\](){}<>",;=|:]+)\s+(?P<protocol>HTTP/\d(?:\.\d)?)\s+(?P<number>\d+)$'"#,
            "\n",
        )
    );
}

#[test]
fn test_a_learned_plugin_reads_every_sample_line() {
    let file = learn("orders", ORDERS).unwrap();
    let plugin = TomlPlugin::parse(&file).unwrap();

    assert!(ORDERS
        .lines()
        .filter(|line| !line.is_empty())
        .all(|line| plugin.read(line).is_some()));
}

#[test]
fn test_a_learned_pattern_styles_the_fields_its_lines_agree_on() {
    let file = learn("orders", ORDERS).unwrap();

    assert_eq!(
        fields(
            &file,
            "2024-03-06 08:00:00,001 DEBUG [checkout] cart 9 emptied"
        ),
        vec![
            pair("timestamp", "2024-03-06 08:00:00,001"),
            pair("level", "DEBUG"),
            pair("message", "cart 9 emptied"),
        ]
    );
}

#[test]
fn test_one_sample_line_is_read_by_one_pattern_to_its_end() {
    assert_eq!(
        learn("deploys", "deploy 12 done\n").unwrap(),
        concat!(
            "name = 'deploys'\n",
            "description = 'Learned from 1 sample line'\n",
            "\n",
            "[patterns]\n",
            r"line = '^deploy\s+(?P<number>\d+)\s+done$'",
            "\n",
        )
    );
}

#[test]
fn test_a_field_seen_twice_is_styled_the_first_time() {
    let file = learn("copies", "copy 1 to 2\n").unwrap();

    assert_eq!(
        patterns(&file),
        vec![r"line = '^copy\s+(?P<number>\d+)\s+to\s+(?:\d+)$'"]
    );
}

#[test]
fn test_addresses_durations_and_email_addresses_are_fields() {
    let file = learn(
        "logins",
        "login ana@example.com 10.0.0.1 120ms\nlogin bo@example.org 10.0.0.2 2.5s\n",
    )
    .unwrap();

    assert_eq!(
        fields(&file, "login cy@example.net 192.168.1.9 15us"),
        vec![
            pair("email", "cy@example.net"),
            pair("ip", "192.168.1.9"),
            pair("duration", "15us"),
        ]
    );
}

#[test]
fn test_syslog_and_common_log_format_timestamps_are_fields() {
    let syslog = learn("cache", "Oct  7 10:00:01 cache warmed\n").unwrap();
    let clf = learn("cache", "05/Mar/2024:10:15:01 +0000 cache warmed\n").unwrap();

    assert_eq!(
        (patterns(&syslog), patterns(&clf)),
        (
            vec![
                r"line = '^(?P<timestamp>[A-Za-z]+\s+\d+\s+\d+:\d+:\d+)\s+cache\s+warmed$'"
                    .to_string()
            ],
            vec![
                r"line = '^(?P<timestamp>\d+/[A-Za-z]+/\d+:\d+:\d+:\d+\s+\+\d+)\s+cache\s+warmed$'"
                    .to_string()
            ]
        )
    );
}

#[test]
fn test_a_line_ending_before_the_others_starts_the_message_where_it_ends() {
    let file = learn("jobs", "job started\njob started late\n").unwrap();

    assert_eq!(
        patterns(&file),
        vec![r"line = '^job\s+started(?P<message>.*)$'"]
    );
}

#[test]
fn test_words_that_differ_start_the_message_unless_they_share_a_form_with_digits() {
    let names = learn("queues", "queue alpha full\nqueue beta full\n").unwrap();
    let forms = learn("queues", "queue q1 full\nqueue q-2 full\n").unwrap();
    let kinds = learn("queues", "queue q1 full\nqueue 7 full\n").unwrap();

    assert_eq!(
        (patterns(&names), patterns(&forms), patterns(&kinds)),
        (
            vec![r"line = '^queue\s+(?P<message>.*)$'".to_string()],
            vec![r"line = '^queue\s+(?P<message>.*)$'".to_string()],
            vec![r"line = '^queue\s+(?P<message>.*)$'".to_string()],
        )
    );
}

#[test]
fn test_a_pattern_holding_a_single_quote_is_written_as_a_quoted_string() {
    let file = learn("notes", "note \"it's\" C:\\temp\n").unwrap();

    assert_eq!(
        (
            patterns(&file),
            fields(&file, "note \"it's\" C:\\temp").is_empty()
        ),
        (
            vec![r#"line = "^note\\s+\"it's\"\\s+C:\\\\temp$""#.to_string()],
            true
        )
    );
}

#[test]
fn test_learning_refuses_the_name_of_a_built_in_plugin() {
    let error = learn("syslog", ORDERS).unwrap_err();

    assert_eq!(
        (error.clone(), error.to_string()),
        (
            LearnError::BuiltinName("syslog".to_string()),
            "'syslog' is the name of a built-in plugin".to_string()
        )
    );
}

#[test]
fn test_learning_from_blank_lines_alone_fails() {
    let error = learn("orders", "\n   \n").unwrap_err();

    assert_eq!(
        (error.clone(), error.to_string()),
        (
            LearnError::NoSamples,
            "there are no sample lines to learn from".to_string()
        )
    );
}
