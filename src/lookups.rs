//! Service lookups
//!
//! With lookups on, splash styles a user name the system does not know in the
//! `unknown_user` color, follows a port with the name of its service, and
//! follows an IPv4 address with the host name reverse DNS gives it and the
//! location a network table gives it. A `[lookups]` config table chooses
//! which lookups run and where they read from.
use crate::output::{ParsedLine, Token, TokenKind};
use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::fs;
use std::net::Ipv4Addr;
use std::sync::{Arc, LazyLock, Mutex};

/// Where service names are read from unless the config names another file
pub const SERVICES_FILE: &str = "/etc/services";

/// The keys a `[lookups]` table can hold
pub const KEYS: [&str; 7] = [
    "enabled", "users", "ports", "dns", "passwd", "services", "geoip",
];

/// Finds the host name for an address
pub type Resolver = fn(Ipv4Addr) -> Option<String>;

/// A port written as `port 22`, or after an address as `10.0.0.5:22`
static PORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bport\s+(\d{1,5})\b|\b\d{1,3}(?:\.\d{1,3}){3}:(\d{1,5})\b").unwrap()
});

/// An IPv4 address, with the port written after it, if any
static ADDRESS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{1,3}(?:\.\d{1,3}){3})(?::\d{1,5}\b)?").unwrap());

/// Error returned when lookups cannot be set up
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
    UnknownKey(String),
    BadSwitch { key: String, value: String },
    Io { path: String, message: String },
    BadNetwork { path: String, line: usize },
}

impl fmt::Display for LookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LookupError::UnknownKey(key) => write!(f, "Unknown lookups key '{}'", key),
            LookupError::BadSwitch { key, value } => write!(
                f,
                "Invalid lookups setting {} = '{}' (expected true or false)",
                key, value
            ),
            LookupError::Io { path, message } => {
                write!(f, "Could not read {}: {}", path, message)
            }
            LookupError::BadNetwork { path, line } => write!(
                f,
                "{}: line {}: expected NETWORK/BITS,COUNTRY,CITY",
                path, line
            ),
        }
    }
}

impl std::error::Error for LookupError {}

/// Which user names count as known
#[derive(Debug, Clone, PartialEq, Eq)]
enum Users {
    /// The names the system's account database holds
    System,
    /// The names a passwd file lists
    Listed(HashSet<String>),
}

/// One network of a location table
#[derive(Debug, Clone, PartialEq, Eq)]
struct Network {
    first: u32,
    last: u32,
    location: String,
}

/// Answers already looked up, kept for the rest of the run
#[derive(Debug, Default)]
struct Answers {
    users: HashMap<String, bool>,
    hosts: HashMap<Ipv4Addr, Option<String>>,
    locations: HashMap<Ipv4Addr, Option<String>>,
}

/// The lookups a run makes
#[derive(Debug, Clone)]
pub struct Lookups {
    users: Option<Users>,
    services: HashMap<u16, String>,
    networks: Vec<Network>,
    resolver: Option<Resolver>,
    answers: Arc<Mutex<Answers>>,
}

impl PartialEq for Lookups {
    fn eq(&self, other: &Self) -> bool {
        self.users == other.users
            && self.services == other.services
            && self.networks == other.networks
            && self.resolver.is_some() == other.resolver.is_some()
    }
}

impl Eq for Lookups {}

impl Default for Lookups {
    fn default() -> Self {
        Lookups::none()
    }
}

impl Lookups {
    /// Looks nothing up
    pub fn none() -> Lookups {
        Lookups {
            users: None,
            services: HashMap::new(),
            networks: Vec::new(),
            resolver: None,
            answers: Arc::default(),
        }
    }

    /// The lookups a `[lookups]` table asks for. `enabled`, from `--lookups`
    /// or `--no-lookups`, wins over the table's `enabled` key, and lookups
    /// are off when neither turns them on.
    pub fn build(
        enabled: Option<bool>,
        configured: &BTreeMap<String, String>,
    ) -> Result<Lookups, LookupError> {
        if let Some(key) = configured.keys().find(|key| !KEYS.contains(&key.as_str())) {
            return Err(LookupError::UnknownKey(key.clone()));
        }

        let switch = |key: &str, default: bool| match configured.get(key).map(String::as_str) {
            None => Ok(default),
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            Some(value) => Err(LookupError::BadSwitch {
                key: key.to_string(),
                value: value.to_string(),
            }),
        };

        let configured_on = switch("enabled", false)?;
        let users = switch("users", true)?;
        let ports = switch("ports", true)?;
        let dns = switch("dns", false)?;

        if !enabled.unwrap_or(configured_on) {
            return Ok(Lookups::none());
        }

        let users = match (users, configured.get("passwd")) {
            (false, _) => None,
            (true, None) => Some(Users::System),
            (true, Some(path)) => Some(Users::Listed(read_passwd(path)?)),
        };

        let services = if ports {
            read_services(
                configured
                    .get("services")
                    .map_or(SERVICES_FILE, |path| path),
            )?
        } else {
            HashMap::new()
        };

        let networks = match configured.get("geoip") {
            Some(path) => read_networks(path)?,
            None => Vec::new(),
        };

        Ok(Lookups {
            users,
            services,
            networks,
            resolver: dns.then_some(reverse_dns as Resolver),
            answers: Arc::default(),
        })
    }

    /// The same lookups, finding host names with `resolver`
    pub fn with_resolver(self, resolver: Resolver) -> Lookups {
        Lookups {
            resolver: Some(resolver),
            ..self
        }
    }

    /// Restyles unknown user names and follows each port with its service
    /// and each address with its host name and location
    pub fn apply<'a>(&self, parsed: ParsedLine<'a>) -> ParsedLine<'a> {
        if *self == Lookups::none() {
            return parsed;
        }

        let mut notes = self.service_notes(&parsed.text());
        let mut offset = 0;

        for token in &parsed.tokens {
            if matches!(token.kind, TokenKind::Ip | TokenKind::Client) {
                if let Some((address, end)) = address_in(&token.text) {
                    notes.extend(self.address_notes(address, offset + end));
                }
            }

            offset += token.text.len();
        }

        notes.sort_by_key(|(at, _)| *at);

        let mut notes = notes.into_iter().peekable();
        let mut tokens = Vec::new();
        let mut offset = 0;

        for token in parsed.tokens {
            let token = self.restyle_user(token);
            let end = offset + token.text.len();
            let mut cut = 0;

            while let Some((at, note)) = notes.next_if(|(at, _)| *at <= end) {
                let at = at - offset;

                if at > cut {
                    tokens.push(token.slice(cut, at));
                    cut = at;
                }

                tokens.push(note);
            }

            if cut < token.text.len() {
                tokens.push(token.slice(cut, token.text.len()));
            }

            offset = end;
        }

        ParsedLine::new(tokens)
    }

    /// A service name after each port with a known service
    fn service_notes(&self, text: &str) -> Vec<(usize, Token<'static>)> {
        PORT.captures_iter(text)
            .filter_map(|found| {
                let port = found.get(1).or_else(|| found.get(2)).unwrap();
                let name = self.services.get(&port.as_str().parse().ok()?)?;

                Some((
                    port.end(),
                    Token::owned(format!("({})", name), TokenKind::Service),
                ))
            })
            .collect()
    }

    /// The host name and location of an address, placed at `at`
    fn address_notes(&self, address: Ipv4Addr, at: usize) -> Vec<(usize, Token<'static>)> {
        let host = self.host(address).map(|host| (host, TokenKind::Host));
        let location = self
            .location(address)
            .map(|location| (location, TokenKind::Location));

        host.into_iter()
            .chain(location)
            .map(|(text, kind)| (at, Token::owned(format!(" ({})", text), kind)))
            .collect()
    }

    fn restyle_user<'a>(&self, token: Token<'a>) -> Token<'a> {
        match &self.users {
            Some(users)
                if token.kind == TokenKind::UserId
                    && token.text != "-"
                    && !self.knows(users, &token.text) =>
            {
                Token {
                    kind: TokenKind::UnknownUser,
                    ..token
                }
            }
            _ => token,
        }
    }

    fn knows(&self, users: &Users, name: &str) -> bool {
        match users {
            Users::Listed(names) => names.contains(name),
            Users::System => *self
                .answers
                .lock()
                .unwrap()
                .users
                .entry(name.to_string())
                .or_insert_with(|| system_user_exists(name)),
        }
    }

    fn host(&self, address: Ipv4Addr) -> Option<String> {
        let resolver = self.resolver?;

        self.answers
            .lock()
            .unwrap()
            .hosts
            .entry(address)
            .or_insert_with(|| resolver(address))
            .clone()
    }

    /// The location of the narrowest network holding the address
    fn location(&self, address: Ipv4Addr) -> Option<String> {
        let value = u32::from(address);

        self.answers
            .lock()
            .unwrap()
            .locations
            .entry(address)
            .or_insert_with(|| {
                self.networks
                    .iter()
                    .filter(|network| network.first <= value && value <= network.last)
                    .min_by_key(|network| network.last - network.first)
                    .map(|network| network.location.clone())
            })
            .clone()
    }
}

/// The first valid IPv4 address in the text, and where it and the port
/// written after it end
fn address_in(text: &str) -> Option<(Ipv4Addr, usize)> {
    let found = ADDRESS.captures(text)?;
    let address = found[1].parse().ok()?;

    Some((address, found.get(0).unwrap().end()))
}

fn read(path: &str) -> Result<String, LookupError> {
    fs::read_to_string(path).map_err(|error| LookupError::Io {
        path: path.to_string(),
        message: error.to_string(),
    })
}

/// The user names a passwd file lists
fn read_passwd(path: &str) -> Result<HashSet<String>, LookupError> {
    Ok(read(path)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.split(':').next().unwrap().to_string())
        .collect())
}

/// The service name for each port a services file lists, taking the first
/// name given to a port
fn read_services(path: &str) -> Result<HashMap<u16, String>, LookupError> {
    let mut services = HashMap::new();

    for (port, name) in read(path)?.lines().filter_map(service_entry) {
        services.entry(port).or_insert(name);
    }

    Ok(services)
}

fn service_entry(line: &str) -> Option<(u16, String)> {
    let mut fields = line.split('#').next().unwrap().split_whitespace();
    let name = fields.next()?;
    let port = fields.next()?.split('/').next().unwrap().parse().ok()?;

    Some((port, name.to_string()))
}

/// The networks a location table lists, one `NETWORK/BITS,COUNTRY,CITY` line
/// each
fn read_networks(path: &str) -> Result<Vec<Network>, LookupError> {
    read(path)?
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty() && !line.trim().starts_with('#'))
        .map(|(index, line)| {
            parse_network(line).ok_or_else(|| LookupError::BadNetwork {
                path: path.to_string(),
                line: index + 1,
            })
        })
        .collect()
}

fn parse_network(line: &str) -> Option<Network> {
    let mut fields = line.split(',').map(str::trim);
    let (address, bits) = fields.next().unwrap().split_once('/')?;
    let address: Ipv4Addr = address.parse().ok()?;
    let bits: u32 = bits.parse().ok().filter(|bits| *bits <= 32)?;
    let country = fields.next()?;
    let city = fields.next().unwrap_or("");
    let mask = u32::MAX.checked_shl(32 - bits).unwrap_or(0);
    let first = u32::from(address) & mask;
    let location: Vec<&str> = [country, city]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect();

    Some(Network {
        first,
        last: first | !mask,
        location: location.join(", "),
    })
}

/// The longest host name `getnameinfo` writes, with its terminating NUL
#[cfg(unix)]
const HOST_NAME_CAPACITY: usize = 1025;

/// The host name the system resolver gives an address, which comes from
/// `/etc/hosts` or a DNS PTR record
#[cfg(unix)]
pub fn reverse_dns(address: Ipv4Addr) -> Option<String> {
    let length = std::mem::size_of::<libc::sockaddr_in>();
    let mut socket: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    socket.sin_family = libc::AF_INET as libc::sa_family_t;
    socket.sin_addr.s_addr = u32::from(address).to_be();

    let mut host = [0 as libc::c_char; HOST_NAME_CAPACITY];
    let status = unsafe {
        libc::getnameinfo(
            &socket as *const libc::sockaddr_in as *const libc::sockaddr,
            length as libc::socklen_t,
            host.as_mut_ptr(),
            host.len() as libc::socklen_t,
            std::ptr::null_mut(),
            0,
            libc::NI_NAMEREQD,
        )
    };

    (status == 0).then(|| {
        unsafe { std::ffi::CStr::from_ptr(host.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    })
}

#[cfg(not(unix))]
pub fn reverse_dns(_address: Ipv4Addr) -> Option<String> {
    None
}

/// Room for the strings `getpwnam_r` copies out of an account entry
#[cfg(unix)]
const ACCOUNT_BUFFER_SIZE: usize = 16384;

/// Whether the system's account database holds this user name
#[cfg(unix)]
pub fn system_user_exists(name: &str) -> bool {
    let Ok(name) = std::ffi::CString::new(name) else {
        return false;
    };

    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut buffer = vec![0 as libc::c_char; ACCOUNT_BUFFER_SIZE];
    let mut found: *mut libc::passwd = std::ptr::null_mut();

    let status = unsafe {
        libc::getpwnam_r(
            name.as_ptr(),
            &mut entry,
            buffer.as_mut_ptr(),
            buffer.len(),
            &mut found,
        )
    };

    status == 0 && !found.is_null()
}

#[cfg(not(unix))]
pub fn system_user_exists(_name: &str) -> bool {
    true
}
