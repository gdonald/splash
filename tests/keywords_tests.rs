use splash::keywords::{restyle, KeywordError, Keywords, BUILT_IN};
use splash::output::{ParsedLine, Token, TokenKind};

fn plain(text: &str) -> ParsedLine<'_> {
    ParsedLine::new(vec![Token::new(text, TokenKind::Plain)])
}

/// The styled pieces of a line, leaving out plain ones
fn styled(parsed: &ParsedLine) -> Vec<(String, TokenKind)> {
    parsed
        .tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Plain)
        .map(|token| (token.text.to_string(), token.kind))
        .collect()
}

fn pairs(entries: &[(&str, &str)]) -> Vec<(String, String)> {
    entries
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[test]
fn the_built_in_keywords_cover_levels_networks_statuses_actions_and_statements() {
    let words: Vec<&str> = BUILT_IN.iter().map(|(word, _)| *word).collect();

    for word in [
        "error", "warn", "tcp", "tls", "success", "failed", "restart", "rollback",
    ] {
        assert!(words.contains(&word), "{}", word);
    }
}

#[test]
fn built_in_keywords_are_styled_in_any_case() {
    let parsed = Keywords::built_in().apply(plain("Error: TCP reset, retry ok"));

    assert_eq!(
        styled(&parsed),
        vec![
            ("Error".to_string(), TokenKind::Failure),
            ("TCP".to_string(), TokenKind::Protocol),
            ("ok".to_string(), TokenKind::Success),
        ]
    );
}

#[test]
fn keywords_match_whole_words_only() {
    let parsed = Keywords::built_in().apply(plain("TCP_MISS errors okay"));

    assert_eq!(styled(&parsed), vec![]);
}

#[test]
fn every_category_has_its_style() {
    let parsed = Keywords::built_in().apply(plain("warning info start select"));

    assert_eq!(
        styled(&parsed),
        vec![
            ("warning".to_string(), TokenKind::Warning),
            ("info".to_string(), TokenKind::Level),
            ("start".to_string(), TokenKind::Keyword),
            ("select".to_string(), TokenKind::Method),
        ]
    );
}

#[test]
fn keywords_are_styled_in_message_text() {
    let parsed = ParsedLine::new(vec![Token::new("query failed", TokenKind::Message)]);

    assert_eq!(
        styled(&Keywords::built_in().apply(parsed))
            .into_iter()
            .filter(|(_, kind)| *kind != TokenKind::Message)
            .collect::<Vec<_>>(),
        vec![("failed".to_string(), TokenKind::Failure)]
    );
}

#[test]
fn keywords_in_a_field_a_parser_styled_are_left_alone() {
    let parsed = ParsedLine::new(vec![Token::new("ERROR", TokenKind::Request)]);

    assert_eq!(Keywords::built_in().apply(parsed.clone()), parsed);
}

#[test]
fn no_keywords_leave_a_line_alone() {
    let parsed = plain("ERROR: tcp failed");

    assert_eq!(Keywords::none().apply(parsed.clone()), parsed);
    assert_eq!(Keywords::default(), Keywords::none());
}

#[test]
fn configured_keywords_are_styled_alongside_the_built_in_ones() {
    let keywords = Keywords::build(
        &pairs(&[
            ("failure", "timed out, Refused"),
            ("success", " deployed ,"),
        ]),
        &[],
    )
    .unwrap();

    let parsed = keywords.apply(plain("deployed; then refused, then timed out ok"));

    assert_eq!(
        styled(&parsed),
        vec![
            ("deployed".to_string(), TokenKind::Success),
            ("refused".to_string(), TokenKind::Failure),
            ("timed out".to_string(), TokenKind::Failure),
            ("ok".to_string(), TokenKind::Success),
        ]
    );
}

#[test]
fn a_configured_keyword_can_restyle_a_built_in_one() {
    let keywords = Keywords::build(&pairs(&[("warning", "failed")]), &[]).unwrap();

    assert_eq!(
        styled(&keywords.apply(plain("failed"))),
        vec![("failed".to_string(), TokenKind::Warning)]
    );
}

#[test]
fn a_configured_keyword_with_punctuation_at_its_ends_matches_without_word_boundaries() {
    let keywords = Keywords::build(&pairs(&[("failure", "!!")]), &[]).unwrap();

    assert_eq!(
        styled(&keywords.apply(plain("disk full!!"))),
        vec![("!!".to_string(), TokenKind::Failure)]
    );
}

#[test]
fn configured_patterns_style_their_matches_over_keywords() {
    let keywords = Keywords::build(
        &[],
        &pairs(&[("transaction", r"req-[0-9a-f]{8}"), ("host", "db failed")]),
    )
    .unwrap();

    assert_eq!(
        styled(&keywords.apply(plain("req-4f2a1c01 db failed"))),
        vec![
            ("req-4f2a1c01".to_string(), TokenKind::Transaction),
            ("db failed".to_string(), TokenKind::Host),
        ]
    );
}

#[test]
fn a_configured_keyword_with_an_unknown_color_key_is_reported() {
    assert_eq!(
        Keywords::build(&pairs(&[("loud", "deploy")]), &[]).unwrap_err(),
        KeywordError::UnknownKind("loud".to_string())
    );
}

#[test]
fn a_configured_pattern_with_an_unknown_color_key_is_reported() {
    assert_eq!(
        Keywords::build(&[], &pairs(&[("loud", "deploy")])).unwrap_err(),
        KeywordError::UnknownKind("loud".to_string())
    );
}

#[test]
fn an_invalid_configured_pattern_is_reported() {
    assert!(matches!(
        Keywords::build(&[], &pairs(&[("host", "(")])).unwrap_err(),
        KeywordError::BadPattern { pattern, .. } if pattern == "("
    ));
}

#[test]
fn keyword_errors_explain_themselves() {
    assert_eq!(
        KeywordError::UnknownKind("loud".to_string()).to_string(),
        "Unknown keyword color 'loud'"
    );
    assert_eq!(
        KeywordError::BadPattern {
            pattern: "(".to_string(),
            message: "unclosed group".to_string(),
        }
        .to_string(),
        "Invalid keyword pattern '(': unclosed group"
    );
}

#[test]
fn keyword_sets_built_from_the_same_lists_are_equal() {
    let patterns = pairs(&[("host", "db[0-9]+")]);

    assert_eq!(
        Keywords::build(&[], &patterns).unwrap(),
        Keywords::build(&[], &patterns).unwrap()
    );
    assert_ne!(
        Keywords::build(&[], &patterns).unwrap(),
        Keywords::built_in()
    );
}

#[test]
fn restyling_splits_a_token_into_runs_of_one_mark() {
    let token = Token::new("aébc", TokenKind::Plain);
    let marks = [
        None,
        Some(TokenKind::Match),
        Some(TokenKind::Match),
        Some(TokenKind::Match),
        None,
    ];

    assert_eq!(
        restyle(&token, &marks),
        vec![
            Token::new("a", TokenKind::Plain),
            Token::new("éb", TokenKind::Match),
            Token::new("c", TokenKind::Plain),
        ]
    );
}

#[test]
fn a_keyword_spanning_adjacent_plain_tokens_styles_each_of_them() {
    let keywords = Keywords::build(&pairs(&[("failure", "timed out")]), &[]).unwrap();
    let parsed = ParsedLine::new(vec![
        Token::new("timed", TokenKind::Plain),
        Token::new(" ", TokenKind::Plain),
        Token::new("out", TokenKind::Plain),
        Token::new("42", TokenKind::Number),
        Token::new(" ok", TokenKind::Plain),
    ]);

    assert_eq!(
        styled(&keywords.apply(parsed)),
        vec![
            ("timed".to_string(), TokenKind::Failure),
            (" ".to_string(), TokenKind::Failure),
            ("out".to_string(), TokenKind::Failure),
            ("42".to_string(), TokenKind::Number),
            ("ok".to_string(), TokenKind::Success),
        ]
    );
}
