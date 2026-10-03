//! Elasticsearch log parsing
//!
//! Elasticsearch's plain text logs follow the log4j pattern
//! `[%d{ISO8601}][%-5p][%-25c{1.}] [%node_name]%marker %m%n`, as in
//! `[2023-10-03T12:00:01,123][INFO ][o.e.n.Node               ] [node-1]
//! started`, for the server log and the search and indexing slow logs. A
//! slow log message lists its values as `name[value]`, such as
//! `took[2.3s]`. The JSON logs Elasticsearch 7 writes, and the ECS JSON logs
//! of Elasticsearch 8, are read by the shared JSON scanner.
use crate::json;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "elasticsearch";

static LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        \[([^\]]+)\]            # date
        \[([A-Z]+)(\s*)\]       # level, padded to five characters
        \[(\S+)(\s*)\]          # logger, padded to twenty-five characters
        \ \[([^\]]*)\]          # node
        (.*)                    # message, with its leading space
        $
        ",
    )
    .unwrap()
});

/// A `name[value]` pair or a bracketed name, such as an index and shard
static BRACKETED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\b([a-z_]+))?(\[)([^\[\]]*)(\])").unwrap());

/// The Elasticsearch log plugin
pub struct ElasticsearchPlugin {
    metadata: PluginMetadata,
}

impl ElasticsearchPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Elasticsearch server and slow logs",
                "splash",
            ),
        }
    }
}

impl Default for ElasticsearchPlugin {
    fn default() -> Self {
        ElasticsearchPlugin::new()
    }
}

impl Plugin for ElasticsearchPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn parse_line<'a>(&self, line: &'a str) -> ParseResult<'a> {
        match parse_line(line) {
            Some(parsed) => ParseResult::Parsed(parsed),
            None => ParseResult::NoMatch,
        }
    }
}

/// The kind a level is colored in
pub fn level_kind(level: &str) -> TokenKind {
    match level {
        "ERROR" | "FATAL" => TokenKind::Failure,
        "WARN" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// The kind a slow log value is colored in, by its name
fn slow_value_kind(name: &str) -> TokenKind {
    match name {
        "took" | "took_millis" => TokenKind::Duration,
        "total_hits" | "total_shards" => TokenKind::Number,
        "source" => TokenKind::Request,
        "id" => TokenKind::Transaction,
        _ => TokenKind::Message,
    }
}

/// The kind a value of a JSON log line is colored in, by its key
pub fn value_kind(key: &str, value: &str) -> Option<TokenKind> {
    match key {
        "timestamp" | "@timestamp" => Some(TokenKind::Timestamp),
        "level" | "log.level" => Some(level_kind(value)),
        "component" | "log.logger" => Some(TokenKind::Module),
        "node.name" | "elasticsearch.node.name" => Some(TokenKind::Host),
        "type" | "event.dataset" | "cluster.name" | "elasticsearch.cluster.name" => {
            Some(TokenKind::Tag)
        }
        "cluster.uuid" | "node.id" | "elasticsearch.cluster.uuid" | "elasticsearch.node.id" => {
            Some(TokenKind::Transaction)
        }
        "message" => Some(TokenKind::Message),
        "took"
        | "took_millis"
        | "elasticsearch.slowlog.took"
        | "elasticsearch.slowlog.took_millis" => Some(TokenKind::Duration),
        "source" | "elasticsearch.slowlog.source" => Some(TokenKind::Request),
        "stacktrace" | "error.stack_trace" | "error.message" => Some(TokenKind::Failure),
        _ => None,
    }
}

/// Parses one Elasticsearch log line, plain text or JSON, or returns `None`
/// when the line is neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_text_line(line).or_else(|| json::parse_object(line, value_kind))
}

/// Parses a plain text log line.
pub fn parse_text_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = LINE.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new("[", TokenKind::Punctuation),
        Token::new(field(1), TokenKind::Timestamp),
        Token::new("]", TokenKind::Punctuation),
        Token::new("[", TokenKind::Punctuation),
        Token::new(field(2), level_kind(field(2))),
    ];

    push_padding(&mut tokens, field(3));
    tokens.extend([
        Token::new("]", TokenKind::Punctuation),
        Token::new("[", TokenKind::Punctuation),
        Token::new(field(4), TokenKind::Module),
    ]);
    push_padding(&mut tokens, field(5));
    tokens.extend([
        Token::new("]", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
        Token::new("[", TokenKind::Punctuation),
    ]);

    if !field(6).is_empty() {
        tokens.push(Token::new(field(6), TokenKind::Host));
    }

    tokens.push(Token::new("]", TokenKind::Punctuation));
    push_message(&mut tokens, field(7));

    Some(ParsedLine::new(tokens))
}

fn push_padding<'a>(tokens: &mut Vec<Token<'a>>, padding: &'a str) {
    if !padding.is_empty() {
        tokens.push(Token::new(padding, TokenKind::Plain));
    }
}

/// Colors a message: each `name[value]` pair by its name, and each bracketed
/// name, such as an index and shard, as a module.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in BRACKETED.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map_or("", |found| found.as_str());
        let name = field(1);
        let value = field(3);

        syslog::push_text(tokens, &message[cursor..whole.start()]);

        let kind = if name.is_empty() {
            TokenKind::Module
        } else {
            tokens.push(Token::new(name, TokenKind::Header));
            slow_value_kind(name)
        };

        tokens.push(Token::new(field(2), TokenKind::Punctuation));

        if !value.is_empty() {
            tokens.push(Token::new(value, kind));
        }

        tokens.push(Token::new(field(4), TokenKind::Punctuation));

        cursor = whole.end();
    }

    syslog::push_text(tokens, &message[cursor..]);
}
