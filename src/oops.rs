//! Linux kernel oops parsing
//!
//! When the kernel hits a bug it prints an oops: a headline such as
//! `BUG: kernel NULL pointer dereference, address: 0000000000000008` or
//! `Oops: 0000 [#1] PREEMPT SMP NOPTI`, the CPU, process, and taint flags,
//! the faulting instruction (`RIP: 0010:ext4_writepages+0x1a/0x40 [ext4]`),
//! the registers, the call trace, the modules that were loaded, and an
//! `---[ end trace ... ]---` line. Each line carries the same prefixes as
//! any dmesg line. This mode reads only the lines of an oops, warning, or
//! panic, so it pulls them out of a whole kernel log.
use crate::dmesg;
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "oops";

/// The lines that make up an oops, a warning, or a panic
static OOPS_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^(?:
            BUG:\ | Oops:\ | Kernel\ panic\ -\ not\ syncing | WARNING:\ CPU:\ | INFO:\ task\ 
            | general\ protection\ fault | Unable\ to\ handle\ kernel | \#PF:\ | PGD\ 
            | CPU:\ \d+\ PID:\ | Hardware\ name:\ | Workqueue:\ | RIP:\ | Code:\ | RSP:\ 
            | R(?:[A-Z]{2}|\d{1,2}):\ | [A-Z]S:\ | CR\d:\ | DR\d:\ | Modules\ linked\ in:
            | Call\ Trace: | ---\[\ end\ 
            | \s*</?(?:TASK|IRQ|NMI|EOI)>
            | \s+(?:\?\ )?[\w.$]+\+0x[0-9a-f]+/0x[0-9a-f]+
        )
        ",
    )
    .unwrap()
});

/// The parts of an oops line that carry a value: the CPU, process id, and
/// command, a field name, a function with its offset, the module it lives
/// in, and a hexadecimal value
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        \b(CPU|PID|Comm)(:)(\ )(\S+)                 # CPU, process id, or command
        |
        \b(Tainted|Hardware\ name|Workqueue|Modules\ linked\ in|RIP|Code|RSP|EFLAGS
            |ORIG_RAX|CR\d|DR\d|[A-Z]S|R(?:[A-Z]{2}|\d{1,2})|address|error_code)(:)   # field name
        |
        ([\w.$]+\+0x[0-9a-f]+/0x[0-9a-f]+)          # function and offset
        |
        (\[)([\w-]+)(\])                            # module
        |
        \b(0x[0-9a-f]+|[0-9a-f]{8,16})\b            # hexadecimal value
        ",
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("BUG", TokenKind::Failure),
        ("Oops", TokenKind::Failure),
        ("Kernel panic - not syncing", TokenKind::Failure),
        ("general protection fault", TokenKind::Failure),
        ("Unable to handle kernel", TokenKind::Failure),
        ("kernel NULL pointer dereference", TokenKind::Failure),
        ("unable to handle page fault", TokenKind::Failure),
        ("soft lockup", TokenKind::Failure),
        ("blocked for more than", TokenKind::Warning),
        ("WARNING", TokenKind::Warning),
        ("INFO", TokenKind::Level),
        ("Call Trace", TokenKind::Header),
        ("end trace", TokenKind::Header),
        ("end Kernel panic", TokenKind::Failure),
    ])
});

/// The kernel oops plugin
pub struct OopsPlugin {
    metadata: PluginMetadata,
}

impl OopsPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Linux kernel oops, warning, and panic messages",
                "splash",
            ),
        }
    }
}

impl Default for OopsPlugin {
    fn default() -> Self {
        OopsPlugin::new()
    }
}

impl Plugin for OopsPlugin {
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

/// Parses one line of an oops, with or without dmesg's prefixes, or returns
/// `None` when the line is not part of one.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, message) = dmesg::split_prefix(line).unwrap_or((Vec::new(), line));

    if !OOPS_LINE.is_match(message) {
        return None;
    }

    push_message(&mut tokens, message);

    Some(ParsedLine::new(tokens))
}

/// Colors one line of an oops.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in SPOTS.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        mail::push_words(tokens, &message[cursor..whole.start()], &WORDS);

        if let Some(name) = field(1) {
            let kind = if name == "Comm" {
                TokenKind::Tag
            } else {
                TokenKind::Number
            };

            tokens.extend([
                Token::new(name, TokenKind::Header),
                Token::new(field(2).unwrap(), TokenKind::Punctuation),
                Token::new(field(3).unwrap(), TokenKind::Plain),
                Token::new(field(4).unwrap(), kind),
            ]);
        } else if let Some(name) = field(5) {
            tokens.extend([
                Token::new(name, TokenKind::Header),
                Token::new(field(6).unwrap(), TokenKind::Punctuation),
            ]);
        } else if let Some(function) = field(7) {
            tokens.push(Token::new(function, TokenKind::Module));
        } else if let Some(module) = field(9) {
            tokens.extend([
                Token::new(field(8).unwrap(), TokenKind::Punctuation),
                Token::new(module, TokenKind::Tag),
                Token::new(field(10).unwrap(), TokenKind::Punctuation),
            ]);
        } else {
            tokens.push(Token::new(field(11).unwrap(), TokenKind::Number));
        }

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}
