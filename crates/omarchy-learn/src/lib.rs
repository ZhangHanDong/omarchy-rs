use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, BufRead, Read, Write},
    net::{IpAddr, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::Url;

const SCHEMA_VERSION: u32 = 1;
const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_AGENT_OUTPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_REDIRECTS: usize = 3;
const AGENT_TIMEOUT: Duration = Duration::from_secs(300);
const PLAN_TTL: Duration = Duration::from_secs(3600);
const MENU_BEGIN: &str = "// BEGIN OMARCHY-RS LEARN (managed; do not edit)";
const MENU_END: &str = "// END OMARCHY-RS LEARN";

#[derive(Clone, Debug)]
pub struct LearnLayout {
    pub home: PathBuf,
    pub config_root: PathBuf,
    pub state_root: PathBuf,
    pub cache_root: PathBuf,
    pub path: Vec<PathBuf>,
    pub agents: BTreeMap<Agent, PathBuf>,
}

impl LearnLayout {
    pub fn from_environment() -> Result<Self, String> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME is unset")?;
        let config_root = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let state_root = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        let cache_root = env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cache"));
        let path = env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect();
        let mut agents = BTreeMap::new();
        agents.insert(
            Agent::Codex,
            env::var_os("OMARCHY_RS_LEARN_CODEX")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("codex")),
        );
        agents.insert(
            Agent::Claude,
            env::var_os("OMARCHY_RS_LEARN_CLAUDE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("claude")),
        );
        agents.insert(
            Agent::Grok,
            env::var_os("OMARCHY_RS_LEARN_GROK")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("grok")),
        );
        agents.insert(
            Agent::Octos,
            env::var_os("OMARCHY_RS_LEARN_OCTOS")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".octos/bin/octos")),
        );
        Ok(Self {
            home,
            config_root,
            state_root,
            cache_root,
            path,
            agents,
        })
    }

    fn learn_config(&self) -> PathBuf {
        self.config_root.join("omarchy-rs/learn")
    }
    fn registry_path(&self) -> PathBuf {
        self.learn_config().join("books.json")
    }
    fn menu_path(&self) -> PathBuf {
        self.config_root
            .join("omarchy/extensions/omarchy-menu.jsonc")
    }
    fn plans_dir(&self) -> PathBuf {
        self.state_root.join("omarchy-rs/learn/plans")
    }
    fn translations_dir(&self) -> PathBuf {
        self.cache_root.join("omarchy-rs/learn/translations")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Codex,
    Claude,
    Grok,
    Octos,
}

impl Agent {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "codex" => Ok(Self::Codex),
            "claude" => Ok(Self::Claude),
            "grok" => Ok(Self::Grok),
            "octos" | "octoscode" => Ok(Self::Octos),
            _ => Err(format!("unsupported Agent: {value}")),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Grok => "grok",
            Self::Octos => "octos",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub id: String,
    pub label: String,
    pub url: String,
    pub description: Option<String>,
    pub source: String,
    pub mutable: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Registry {
    schema_version: u32,
    books: Vec<UserBook>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserBook {
    id: String,
    label: String,
    url: String,
    description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BooksReport {
    schema_version: u32,
    books: Vec<Book>,
    agent_availability: BTreeMap<Agent, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationPlan {
    pub schema_version: u32,
    pub id: String,
    pub confirmation_token: String,
    pub book_id: String,
    pub book_label: String,
    pub source_url: String,
    pub source_identity: String,
    pub agent: Agent,
    pub language: String,
    pub maximum_source_bytes: usize,
    pub cache_key: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationResult {
    schema_version: u32,
    book_id: String,
    source_url: String,
    agent: Agent,
    language: String,
    cached: bool,
    output_path: PathBuf,
}

fn builtin_books() -> Vec<Book> {
    [
        ("omarchy", "Omarchy Manual", "https://omarchy.org/manual/"),
        ("hyprland", "Hyprland Wiki", "https://wiki.hypr.land/"),
        (
            "arch",
            "Arch Wiki",
            "https://wiki.archlinux.org/title/Main_page",
        ),
        (
            "neovim",
            "LazyVim Keymaps",
            "https://www.lazyvim.org/keymaps",
        ),
        ("bash", "Bash Cheatsheet", "https://devhints.io/bash"),
    ]
    .into_iter()
    .map(|(id, label, url)| Book {
        id: id.into(),
        label: label.into(),
        url: url.into(),
        description: None,
        source: "omarchy-built-in".into(),
        mutable: false,
    })
    .collect()
}

fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn validate_label(label: &str) -> Result<(), String> {
    if label.trim() != label || label.is_empty() || label.chars().count() > 120 {
        return Err("Book label must be 1-120 trimmed characters".into());
    }
    if label.chars().any(char::is_control) {
        return Err("Book label contains control characters".into());
    }
    Ok(())
}

fn parse_public_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "Book URL is invalid")?;
    if url.scheme() != "https" {
        return Err("Book URL must use HTTPS".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Book URL must not contain credentials".into());
    }
    if url.query_pairs().any(|(key, _)| {
        matches!(
            key.to_ascii_lowercase().as_str(),
            "token" | "key" | "api_key" | "apikey" | "password" | "secret"
        )
    }) {
        return Err("Book URL contains a credential-like query".into());
    }
    let host = url.host_str().ok_or("Book URL has no hostname")?;
    if host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".localhost")
        || host.parse::<IpAddr>().is_ok()
    {
        return Err("Book URL hostname is local or an IP literal".into());
    }
    Ok(url)
}

fn load_registry(layout: &LearnLayout) -> Result<Registry, String> {
    let path = layout.registry_path();
    if !path.exists() {
        return Ok(Registry {
            schema_version: SCHEMA_VERSION,
            books: Vec::new(),
        });
    }
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let mut registry: Registry =
        serde_json::from_slice(&bytes).map_err(|_| "invalid Learn Book registry")?;
    if registry.schema_version != SCHEMA_VERSION {
        return Err("unsupported Learn Book registry schema".into());
    }
    registry.books.sort_by(|left, right| left.id.cmp(&right.id));
    for book in &registry.books {
        validate_user_book(book)?;
    }
    Ok(registry)
}

fn validate_user_book(book: &UserBook) -> Result<(), String> {
    if !valid_id(&book.id) {
        return Err("Book id must be lowercase letters, digits, and interior hyphens".into());
    }
    if builtin_books()
        .iter()
        .any(|built_in| built_in.id == book.id)
    {
        return Err("Book id conflicts with an Omarchy built-in".into());
    }
    validate_label(&book.label)?;
    parse_public_url(&book.url)?;
    if book
        .description
        .as_ref()
        .is_some_and(|value| value.chars().count() > 240 || value.chars().any(char::is_control))
    {
        return Err("Book description exceeds 240 characters or contains controls".into());
    }
    Ok(())
}

fn all_books(layout: &LearnLayout) -> Result<Vec<Book>, String> {
    let registry = load_registry(layout)?;
    let mut books = builtin_books();
    books.extend(registry.books.into_iter().map(|book| Book {
        id: book.id,
        label: book.label,
        url: book.url,
        description: book.description,
        source: "user".into(),
        mutable: true,
    }));
    books.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(books)
}

pub fn books(layout: &LearnLayout) -> Result<BooksReport, String> {
    let mut availability = BTreeMap::new();
    for agent in [Agent::Codex, Agent::Claude, Agent::Grok] {
        availability.insert(
            agent,
            if resolve_executable(layout, agent).is_some() {
                "available"
            } else {
                "unavailable"
            }
            .into(),
        );
    }
    availability.insert(Agent::Octos, "unsupported-single-turn-interface".into());
    Ok(BooksReport {
        schema_version: SCHEMA_VERSION,
        books: all_books(layout)?,
        agent_availability: availability,
    })
}

pub fn add_book(
    layout: &LearnLayout,
    id: &str,
    label: &str,
    url: &str,
    description: Option<&str>,
) -> Result<BooksReport, String> {
    let candidate = UserBook {
        id: id.into(),
        label: label.into(),
        url: url.into(),
        description: description.map(str::to_owned),
    };
    validate_user_book(&candidate)?;
    let mut registry = load_registry(layout)?;
    if registry.books.iter().any(|book| book.id == id) {
        return Err("Book id already exists".into());
    }
    registry.books.push(candidate);
    registry.books.sort_by(|left, right| left.id.cmp(&right.id));
    atomic_json(&layout.registry_path(), &registry)?;
    books(layout)
}

pub fn remove_book(layout: &LearnLayout, id: &str) -> Result<BooksReport, String> {
    if !valid_id(id) {
        return Err("invalid Book id".into());
    }
    let mut registry = load_registry(layout)?;
    let before = registry.books.len();
    registry.books.retain(|book| book.id != id);
    if registry.books.len() == before {
        return Err("user Book not found".into());
    }
    atomic_json(&layout.registry_path(), &registry)?;
    books(layout)
}

fn find_book(layout: &LearnLayout, id: &str) -> Result<Book, String> {
    all_books(layout)?
        .into_iter()
        .find(|book| book.id == id)
        .ok_or_else(|| "Book not found".into())
}

fn menu_entry(id: &str, label: &str, action: &str, description: Option<&str>) -> String {
    let value = serde_json::json!({
        "icon": "",
        "label": label,
        "description": description,
        "action": action,
    });
    let mut object = value.as_object().unwrap().clone();
    if description.is_none() {
        object.remove("description");
    }
    format!(
        "  {}: {},\n",
        serde_json::to_string(id).unwrap(),
        serde_json::to_string(&object).unwrap()
    )
}

fn managed_menu_block(layout: &LearnLayout, prefix_comma: bool) -> Result<String, String> {
    let all = all_books(layout)?;
    let users = all.iter().filter(|book| book.mutable).collect::<Vec<_>>();
    let mut block = String::new();
    block.push_str("  ");
    block.push_str(MENU_BEGIN);
    block.push('\n');
    if prefix_comma {
        block.push_str("  ,\n");
    }
    for book in users {
        block.push_str(&menu_entry(
            &format!("learn.book-{}", book.id),
            &book.label,
            &format!("omarchy-rs learn open --book {}", book.id),
            book.description.as_deref(),
        ));
    }
    block
        .push_str("  \"learn.agent-translate\": {\"icon\":\"󰚩\",\"label\":\"Agent Translate\"},\n");
    for agent in [Agent::Codex, Agent::Claude, Agent::Grok] {
        block.push_str(&format!(
            "  \"learn.agent-translate.{}\": {{\"icon\":\"󰚩\",\"label\":\"{}\"}},\n",
            agent.name(),
            match agent {
                Agent::Codex => "Codex",
                Agent::Claude => "Claude",
                Agent::Grok => "Grok",
                Agent::Octos => unreachable!(),
            }
        ));
        for book in &all {
            block.push_str(&menu_entry(
                &format!("learn.agent-translate.{}.{}", agent.name(), book.id),
                &book.label,
                &format!(
                    "omarchy-launch-floating-terminal-with-presentation \"omarchy-rs learn translate --book {} --agent {} --language zh-CN\"",
                    book.id,
                    agent.name()
                ),
                Some("Translate one chapter to Chinese with explicit confirmation"),
            ));
        }
    }
    block.push_str("  ");
    block.push_str(MENU_END);
    block.push('\n');
    Ok(block)
}

fn marker_ranges(text: &str) -> Result<Option<(usize, usize)>, String> {
    let begins = text.match_indices(MENU_BEGIN).collect::<Vec<_>>();
    let ends = text.match_indices(MENU_END).collect::<Vec<_>>();
    match (begins.len(), ends.len()) {
        (0, 0) => Ok(None),
        (1, 1) if begins[0].0 < ends[0].0 => {
            let line_start = text[..begins[0].0].rfind('\n').map_or(0, |index| index + 1);
            let line_end = text[ends[0].0 + MENU_END.len()..]
                .find('\n')
                .map_or(text.len(), |index| ends[0].0 + MENU_END.len() + index + 1);
            Ok(Some((line_start, line_end)))
        }
        _ => Err("corrupt omarchy-rs Learn menu markers".into()),
    }
}

fn validate_menu_shape(text: &str) -> Result<usize, String> {
    let trimmed = text.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err("user Omarchy menu is not a JSONC object".into());
    }
    text.rfind('}')
        .ok_or_else(|| "user Omarchy menu has no closing brace".into())
}

pub fn sync_menu(layout: &LearnLayout) -> Result<PathBuf, String> {
    let path = layout.menu_path();
    let original = if path.exists() {
        fs::read_to_string(&path).map_err(|error| error.to_string())?
    } else {
        "{\n}\n".into()
    };
    let without = if let Some((start, end)) = marker_ranges(&original)? {
        format!("{}{}", &original[..start], &original[end..])
    } else {
        original
    };
    let closing = validate_menu_shape(&without)?;
    let prior = without[..closing]
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("//"))
        .unwrap_or("{");
    let prefix_comma = prior != "{" && !prior.ends_with(',');
    let block = managed_menu_block(layout, prefix_comma)?;
    let mut updated = String::with_capacity(without.len() + block.len());
    updated.push_str(&without[..closing]);
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&block);
    updated.push_str(&without[closing..]);
    atomic_bytes(&path, updated.as_bytes())?;
    Ok(path)
}

pub fn unsync_menu(layout: &LearnLayout) -> Result<PathBuf, String> {
    let path = layout.menu_path();
    let original = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let Some((start, end)) = marker_ranges(&original)? else {
        return Ok(path);
    };
    let updated = format!("{}{}", &original[..start], &original[end..]);
    validate_menu_shape(&updated)?;
    atomic_bytes(&path, updated.as_bytes())?;
    Ok(path)
}

fn book_identity(book: &Book) -> Result<String, String> {
    let bytes = serde_json::to_vec(book).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn cache_key(identity: &str, agent: Agent, language: &str) -> String {
    let value = format!("{identity}:{}:{language}", agent.name());
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn valid_language(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 35
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub fn create_plan(
    layout: &LearnLayout,
    book_id: &str,
    agent: Agent,
    language: &str,
) -> Result<TranslationPlan, String> {
    if agent == Agent::Octos {
        return Err("octos-unavailable: no safe public single-turn interface".into());
    }
    if resolve_executable(layout, agent).is_none() {
        return Err(format!("{}-unavailable", agent.name()));
    }
    if !valid_language(language) {
        return Err("language must be a short BCP-47-like identifier".into());
    }
    let book = find_book(layout, book_id)?;
    parse_public_url(&book.url)?;
    let source_identity = book_identity(&book)?;
    let created_at_ms = now_ms()?;
    let cache_key = cache_key(&source_identity, agent, language);
    let seed = format!(
        "{created_at_ms}:{}:{}:{language}:{source_identity}",
        book.id,
        agent.name()
    );
    let id = format!("{:x}", Sha256::digest(seed.as_bytes()));
    let confirmation_seed = format!("confirm:{id}:{source_identity}");
    let confirmation_token = format!("{:x}", Sha256::digest(confirmation_seed.as_bytes()));
    let plan = TranslationPlan {
        schema_version: SCHEMA_VERSION,
        id,
        confirmation_token,
        book_id: book.id,
        book_label: book.label,
        source_url: book.url,
        source_identity,
        agent,
        language: language.into(),
        maximum_source_bytes: MAX_SOURCE_BYTES,
        cache_key,
        created_at_ms,
    };
    atomic_json(&layout.plans_dir().join(format!("{}.json", plan.id)), &plan)?;
    Ok(plan)
}

fn load_plan(layout: &LearnLayout, id: &str, token: &str) -> Result<TranslationPlan, String> {
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid translation plan id".into());
    }
    let bytes = fs::read(layout.plans_dir().join(format!("{id}.json")))
        .map_err(|_| "translation plan not found")?;
    let plan: TranslationPlan =
        serde_json::from_slice(&bytes).map_err(|_| "invalid translation plan")?;
    if plan.schema_version != SCHEMA_VERSION || plan.id != id {
        return Err("translation plan identity mismatch".into());
    }
    if plan.confirmation_token != token {
        return Err("confirmation token does not match translation plan".into());
    }
    let age = now_ms()?.saturating_sub(plan.created_at_ms);
    if age > PLAN_TTL.as_millis() as u64 {
        return Err("translation plan expired".into());
    }
    let current = find_book(layout, &plan.book_id)?;
    if book_identity(&current)? != plan.source_identity || current.url != plan.source_url {
        return Err("Book registry changed after planning".into());
    }
    Ok(plan)
}

#[derive(Clone, Debug)]
struct FetchedDocument {
    final_url: String,
    content_type: String,
    bytes: Vec<u8>,
}

trait Fetcher {
    fn fetch(&self, url: &Url) -> Result<FetchedDocument, String>;
}

struct NetworkFetcher;

impl Fetcher for NetworkFetcher {
    fn fetch(&self, initial: &Url) -> Result<FetchedDocument, String> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(20))
            .timeout_write(Duration::from_secs(20))
            .redirects(0)
            .build();
        let mut current = initial.clone();
        for redirect in 0..=MAX_REDIRECTS {
            validate_resolved_host(&current)?;
            let response = match agent
                .get(current.as_str())
                .set("Accept", "text/html,text/plain,text/markdown;q=0.9")
                .call()
            {
                Ok(response) => response,
                Err(ureq::Error::Status(_, response)) => response,
                Err(error) => return Err(format!("Book fetch failed: {error}")),
            };
            let status = response.status();
            if matches!(status, 301 | 302 | 303 | 307 | 308) {
                if redirect == MAX_REDIRECTS {
                    return Err("Book fetch exceeded redirect limit".into());
                }
                let location = response
                    .header("Location")
                    .ok_or("Book redirect has no Location")?;
                current = current
                    .join(location)
                    .map_err(|_| "Book redirect URL is invalid")?;
                parse_public_url(current.as_str())?;
                continue;
            }
            if !(200..300).contains(&status) {
                return Err(format!("Book fetch returned HTTP {status}"));
            }
            let content_type = response
                .header("Content-Type")
                .unwrap_or("text/plain")
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            if !matches!(
                content_type.as_str(),
                "text/html" | "text/plain" | "text/markdown" | "text/x-markdown"
            ) {
                return Err(format!("unsupported Book content type: {content_type}"));
            }
            let mut bytes = Vec::new();
            response
                .into_reader()
                .take((MAX_SOURCE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            if bytes.len() > MAX_SOURCE_BYTES {
                return Err("Book chapter exceeds 1 MiB".into());
            }
            return Ok(FetchedDocument {
                final_url: current.into(),
                content_type,
                bytes,
            });
        }
        Err("unreachable redirect state".into())
    }
}

fn validate_resolved_host(url: &Url) -> Result<(), String> {
    parse_public_url(url.as_str())?;
    let host = url.host_str().ok_or("Book URL has no hostname")?;
    let port = url.port_or_known_default().ok_or("Book URL has no port")?;
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|_| "Book hostname cannot be resolved")?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err("Book hostname resolved to no addresses".into());
    }
    let addresses = addresses
        .into_iter()
        .map(|address| address.ip())
        .collect::<Vec<_>>();
    if !resolved_addresses_are_safe(&addresses, mihomo_interface_exists()) {
        return Err("Book hostname resolves to a private or local address".into());
    }
    Ok(())
}

fn resolved_addresses_are_safe(addresses: &[IpAddr], mihomo_active: bool) -> bool {
    !addresses.is_empty()
        && addresses
            .iter()
            .all(|ip| is_public_ip(*ip) || (mihomo_active && is_mihomo_fake_ip(*ip)))
}

fn mihomo_interface_exists() -> bool {
    Path::new("/sys/class/net/Mihomo").is_dir()
}

fn is_mihomo_fake_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            octets[0] == 198 && matches!(octets[1], 18 | 19)
        }
        IpAddr::V6(ip) => ip.segments()[..3] == [0xfdfe, 0xdcba, 0x9876],
    }
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || octets[0] == 0
                || octets[0] >= 224
                || (octets[0] == 198 && matches!(octets[1], 18 | 19)))
        }
        IpAddr::V6(ip) => {
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.segments()[0] & 0xff00 == 0xff00)
        }
    }
}

fn normalize_document(document: &FetchedDocument) -> Result<String, String> {
    let raw = std::str::from_utf8(&document.bytes).map_err(|_| "Book chapter is not UTF-8")?;
    let text = if document.content_type == "text/html" {
        html_to_text(raw)
    } else {
        raw.to_owned()
    };
    let normalized = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if normalized.is_empty() {
        return Err("Book chapter contains no translatable text".into());
    }
    Ok(normalized)
}

fn html_to_text(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut output = String::with_capacity(html.len().min(MAX_SOURCE_BYTES));
    let mut index = 0usize;
    let bytes = html.as_bytes();
    while index < bytes.len() {
        if lower[index..].starts_with("<script") || lower[index..].starts_with("<style") {
            let closing = if lower[index..].starts_with("<script") {
                "</script>"
            } else {
                "</style>"
            };
            if let Some(end) = lower[index..].find(closing) {
                index += end + closing.len();
                output.push('\n');
                continue;
            }
            break;
        }
        if bytes[index] == b'<' {
            if let Some(end) = html[index..].find('>') {
                let tag = lower[index + 1..index + end]
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if matches!(
                    tag,
                    "p" | "br"
                        | "div"
                        | "section"
                        | "article"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "li"
                        | "pre"
                        | "blockquote"
                        | "tr"
                ) {
                    output.push('\n');
                }
                index += end + 1;
                continue;
            }
        }
        let ch = html[index..].chars().next().unwrap();
        output.push(ch);
        index += ch.len_utf8();
    }
    decode_entities(&output)
}

fn decode_entities(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn resolve_executable(layout: &LearnLayout, agent: Agent) -> Option<PathBuf> {
    let configured = layout.agents.get(&agent)?;
    if configured.components().count() > 1 || configured.is_absolute() {
        return configured.is_file().then(|| configured.clone());
    }
    layout
        .path
        .iter()
        .map(|root| root.join(configured))
        .find(|candidate| candidate.is_file())
}

struct AgentInvocation {
    executable: PathBuf,
    args: Vec<String>,
    stdin: Option<Vec<u8>>,
    working_directory: PathBuf,
}

fn translation_prompt(plan: &TranslationPlan, document: &FetchedDocument, text: &str) -> String {
    format!(
        "Translate the following single public documentation chapter into {}. Preserve headings, code blocks, commands, links, technical identifiers, warnings, and meaning. Do not summarize, browse, call tools, or add commentary. Return Markdown only.\n\nSource: {}\n\n<source_document>\n{}\n</source_document>\n",
        plan.language, document.final_url, text
    )
}

fn agent_invocation(
    layout: &LearnLayout,
    plan: &TranslationPlan,
    prompt: String,
) -> Result<AgentInvocation, String> {
    let executable = resolve_executable(layout, plan.agent)
        .ok_or_else(|| format!("{}-unavailable", plan.agent.name()))?;
    let working_directory = layout
        .state_root
        .join("omarchy-rs/learn/runtime")
        .join(&plan.id);
    fs::create_dir_all(&working_directory).map_err(|error| error.to_string())?;
    match plan.agent {
        Agent::Codex => Ok(AgentInvocation {
            executable,
            args: vec![
                "exec".into(),
                "--ephemeral".into(),
                "--sandbox".into(),
                "read-only".into(),
                "--skip-git-repo-check".into(),
                "--disable".into(),
                "standalone_web_search".into(),
                "--disable".into(),
                "web_search_request".into(),
                "--disable".into(),
                "web_search_cached".into(),
                "--color".into(),
                "never".into(),
                "-C".into(),
                working_directory.display().to_string(),
                "-".into(),
            ],
            stdin: Some(prompt.into_bytes()),
            working_directory,
        }),
        Agent::Claude => Ok(AgentInvocation {
            executable,
            args: [
                "--print",
                "--no-session-persistence",
                "--disable-slash-commands",
                "--tools",
                "",
                "--output-format",
                "text",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            stdin: Some(prompt.into_bytes()),
            working_directory,
        }),
        Agent::Grok => Ok(AgentInvocation {
            executable,
            args: vec![
                "--prompt-file".into(),
                "/dev/stdin".into(),
                "--tools".into(),
                "".into(),
                "--disable-web-search".into(),
                "--max-turns".into(),
                "1".into(),
                "--output-format".into(),
                "plain".into(),
                "--permission-mode".into(),
                "plan".into(),
            ],
            stdin: Some(prompt.into_bytes()),
            working_directory,
        }),
        Agent::Octos => Err("octos-unavailable: no safe public single-turn interface".into()),
    }
}

fn read_bounded<R: Read>(mut reader: R, limit: usize, overflow: Arc<AtomicBool>) -> Vec<u8> {
    let mut output = Vec::new();
    let mut total = 0usize;
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                total = total.saturating_add(count);
                if output.len() < limit {
                    let retained = count.min(limit - output.len());
                    output.extend_from_slice(&buffer[..retained]);
                }
                if total > limit {
                    overflow.store(true, Ordering::Relaxed);
                }
            }
        }
    }
    output
}

fn run_agent(invocation: AgentInvocation) -> Result<String, String> {
    let mut command = Command::new(&invocation.executable);
    command
        .args(&invocation.args)
        .current_dir(&invocation.working_directory)
        .stdin(if invocation.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1");
    let mut child = command
        .spawn()
        .map_err(|error| format!("Agent process failed to start: {error}"))?;
    if let Some(input) = invocation.stdin {
        child
            .stdin
            .take()
            .ok_or("Agent stdin is unavailable")?
            .write_all(&input)
            .map_err(|error| error.to_string())?;
    }
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = child.stdout.take().ok_or("Agent stdout is unavailable")?;
    let stderr = child.stderr.take().ok_or("Agent stderr is unavailable")?;
    let stdout_overflow = Arc::clone(&overflow);
    let stderr_overflow = Arc::clone(&overflow);
    let stdout_thread =
        thread::spawn(move || read_bounded(stdout, MAX_AGENT_OUTPUT_BYTES, stdout_overflow));
    let stderr_thread = thread::spawn(move || read_bounded(stderr, 64 * 1024, stderr_overflow));
    let started = Instant::now();
    let status = loop {
        if overflow.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            break Err("Agent output exceeded 2 MiB".to_string());
        }
        if started.elapsed() > AGENT_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            break Err("Agent translation exceeded five minutes".to_string());
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(error) => break Err(error.to_string()),
        }
    };
    let stdout = stdout_thread
        .join()
        .map_err(|_| "Agent stdout reader panicked")?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| "Agent stderr reader panicked")?;
    let status = status?;
    if !status.success() {
        return Err(format!(
            "Agent exited with {} (stderr {} bytes)",
            status.code().unwrap_or(-1),
            stderr.len()
        ));
    }
    let output = String::from_utf8(stdout).map_err(|_| "Agent output is not UTF-8")?;
    if output.trim().is_empty() {
        return Err("Agent returned an empty translation".into());
    }
    Ok(output)
}

pub fn apply_plan(
    layout: &LearnLayout,
    id: &str,
    token: &str,
) -> Result<TranslationResult, String> {
    apply_with(layout, id, token, &NetworkFetcher, |invocation| {
        run_agent(invocation)
    })
}

fn apply_with<F: Fetcher, R: FnOnce(AgentInvocation) -> Result<String, String>>(
    layout: &LearnLayout,
    id: &str,
    token: &str,
    fetcher: &F,
    runner: R,
) -> Result<TranslationResult, String> {
    let plan = load_plan(layout, id, token)?;
    let output_path = layout
        .translations_dir()
        .join(format!("{}.html", plan.cache_key));
    if output_path.is_file() {
        return Ok(translation_result(&plan, output_path, true));
    }
    let url = parse_public_url(&plan.source_url)?;
    let document = fetcher.fetch(&url)?;
    parse_public_url(&document.final_url)?;
    if document.bytes.len() > MAX_SOURCE_BYTES {
        return Err("Book chapter exceeds 1 MiB".into());
    }
    if !matches!(
        document.content_type.as_str(),
        "text/html" | "text/plain" | "text/markdown" | "text/x-markdown"
    ) {
        return Err("unsupported Book content type".into());
    }
    let text = normalize_document(&document)?;
    let prompt = translation_prompt(&plan, &document, &text);
    let invocation = agent_invocation(layout, &plan, prompt)?;
    let translated = runner(invocation)?;
    if translated.len() > MAX_AGENT_OUTPUT_BYTES {
        return Err("Agent output exceeded 2 MiB".into());
    }
    let html = render_translation_html(&plan, &document.final_url, &translated)?;
    atomic_bytes(&output_path, html.as_bytes())?;
    Ok(translation_result(&plan, output_path, false))
}

fn translation_result(
    plan: &TranslationPlan,
    output_path: PathBuf,
    cached: bool,
) -> TranslationResult {
    TranslationResult {
        schema_version: SCHEMA_VERSION,
        book_id: plan.book_id.clone(),
        source_url: plan.source_url.clone(),
        agent: plan.agent,
        language: plan.language.clone(),
        cached,
        output_path,
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn render_translation_html(
    plan: &TranslationPlan,
    final_url: &str,
    translated: &str,
) -> Result<String, String> {
    let generated = chrono::Utc::now().to_rfc3339();
    Ok(format!(
        "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>body{{max-width:900px;margin:3rem auto;padding:0 1.5rem;background:#151311;color:#e8e1d9;font:18px/1.65 system-ui,sans-serif}}a{{color:#ff6a1a}}header{{border-bottom:1px solid #5c4033;margin-bottom:2rem}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;font:inherit}}.meta{{color:#a99f96;font-size:.85rem}}</style></head><body><header><h1>{}</h1><p class=\"meta\">Translated by {} · {} · {} · <a href=\"{}\">Original source</a></p></header><main><pre>{}</pre></main></body></html>",
        html_escape(&plan.language),
        html_escape(&plan.book_label),
        html_escape(&plan.book_label),
        html_escape(plan.agent.name()),
        html_escape(&plan.language),
        html_escape(&generated),
        html_escape(final_url),
        html_escape(translated)
    ))
}

pub fn open_book(layout: &LearnLayout, id: &str) -> Result<String, String> {
    let book = find_book(layout, id)?;
    let status = Command::new("omarchy-launch-webapp")
        .arg(&book.url)
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!(
            "omarchy-launch-webapp exited with {}",
            status.code().unwrap_or(-1)
        ));
    }
    Ok(format!("opened {}", book.url))
}

pub fn open_translation(path: &Path) -> Result<String, String> {
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    if canonical.extension().and_then(|value| value.to_str()) != Some("html") {
        return Err("translation output must be HTML".into());
    }
    let url = Url::from_file_path(&canonical).map_err(|_| "invalid translation path")?;
    let status = Command::new("omarchy-launch-webapp")
        .arg(url.as_str())
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err("failed to open translated chapter".into());
    }
    Ok(format!("opened {}", canonical.display()))
}

fn interactive_translate(
    layout: &LearnLayout,
    book: &str,
    agent: Agent,
    language: &str,
) -> Result<String, String> {
    let plan = create_plan(layout, book, agent, language)?;
    println!(
        "Translate '{}' with {} to {}?\nSource: {}\nMaximum source: {} bytes\nType YES to confirm:",
        plan.book_label,
        plan.agent.name(),
        plan.language,
        plan.source_url,
        plan.maximum_source_bytes
    );
    let mut answer = String::new();
    io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|error| error.to_string())?;
    if answer.trim() != "YES" {
        return Ok("translation cancelled".into());
    }
    let result = apply_plan(layout, &plan.id, &plan.confirmation_token)?;
    open_translation(&result.output_path)?;
    Ok(format!("translated {}", result.output_path.display()))
}

fn now_ms() -> Result<u64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis() as u64)
}

fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    atomic_bytes(path, &bytes)
}

fn atomic_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("learn"),
        std::process::id()
    ));
    if temp.exists() {
        fs::remove_file(&temp).map_err(|error| error.to_string())?;
    }
    let result = (|| -> io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map_err(|error| error.to_string())
}

pub fn execute_cli(args: &[String]) -> Result<String, String> {
    let layout = LearnLayout::from_environment()?;
    match args.first().map(String::as_str) {
        Some("books") if args.len() == 1 || args.get(1).map(String::as_str) == Some("--json") => {
            json(&books(&layout)?)
        }
        Some("add") => {
            let values = parse_flags(&args[1..])?;
            let id = one(&values, "--id")?;
            let label = one(&values, "--label")?;
            let url = one(&values, "--url")?;
            let description = values
                .get("--description")
                .and_then(|items| items.first())
                .map(String::as_str);
            json(&add_book(&layout, id, label, url, description)?)
        }
        Some("remove") => {
            let values = parse_flags(&args[1..])?;
            json(&remove_book(&layout, one(&values, "--id")?)?)
        }
        Some("sync-menu") if args.len() == 1 => {
            Ok(format!("synchronized {}", sync_menu(&layout)?.display()))
        }
        Some("unsync-menu") if args.len() == 1 => Ok(format!(
            "unsynchronized {}",
            unsync_menu(&layout)?.display()
        )),
        Some("plan") => {
            let values = parse_flags(&args[1..])?;
            let agent = Agent::parse(one(&values, "--agent")?)?;
            json(&create_plan(
                &layout,
                one(&values, "--book")?,
                agent,
                values
                    .get("--language")
                    .and_then(|items| items.first())
                    .map(String::as_str)
                    .unwrap_or("zh-CN"),
            )?)
        }
        Some("apply") => {
            let values = parse_flags(&args[1..])?;
            json(&apply_plan(
                &layout,
                one(&values, "--plan")?,
                one(&values, "--confirm")?,
            )?)
        }
        Some("translate") => {
            let values = parse_flags(&args[1..])?;
            interactive_translate(
                &layout,
                one(&values, "--book")?,
                Agent::parse(one(&values, "--agent")?)?,
                values
                    .get("--language")
                    .and_then(|items| items.first())
                    .map(String::as_str)
                    .unwrap_or("zh-CN"),
            )
        }
        Some("open") => {
            let values = parse_flags(&args[1..])?;
            open_book(&layout, one(&values, "--book")?)
        }
        Some("open-translation") => {
            let values = parse_flags(&args[1..])?;
            open_translation(Path::new(one(&values, "--path")?))
        }
        _ => Err(learn_usage()),
    }
}

fn json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|error| error.to_string())
}

fn parse_flags(args: &[String]) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut output = BTreeMap::<String, Vec<String>>::new();
    let mut index = 0;
    while index < args.len() {
        let flag = &args[index];
        if !flag.starts_with("--") || index + 1 >= args.len() {
            return Err(learn_usage());
        }
        output
            .entry(flag.clone())
            .or_default()
            .push(args[index + 1].clone());
        index += 2;
    }
    Ok(output)
}

fn one<'a>(values: &'a BTreeMap<String, Vec<String>>, flag: &str) -> Result<&'a str, String> {
    match values.get(flag).map(Vec::as_slice) {
        Some([value]) => Ok(value),
        _ => Err(format!("missing or repeated {flag}\n{}", learn_usage())),
    }
}

fn learn_usage() -> String {
    "usage: omarchy-rs learn <books [--json]|add --id ID --label LABEL --url HTTPS_URL [--description TEXT]|remove --id ID|sync-menu|unsync-menu|plan --book ID --agent codex|claude|grok --language zh-CN|apply --plan ID --confirm TOKEN|translate --book ID --agent codex|claude|grok [--language zh-CN]|open --book ID|open-translation --path FILE>".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use tempfile::TempDir;

    struct Fixture {
        _temp: TempDir,
        layout: LearnLayout,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let home = temp.path().join("home");
            let bin = temp.path().join("bin");
            fs::create_dir_all(&home).unwrap();
            fs::create_dir_all(&bin).unwrap();
            let mut agents = BTreeMap::new();
            for agent in [Agent::Codex, Agent::Claude, Agent::Grok, Agent::Octos] {
                let executable = bin.join(agent.name());
                fs::write(&executable, "fixture").unwrap();
                agents.insert(agent, executable);
            }
            Self {
                layout: LearnLayout {
                    config_root: home.join(".config"),
                    state_root: home.join(".local/state"),
                    cache_root: home.join(".cache"),
                    path: vec![bin],
                    agents,
                    home,
                },
                _temp: temp,
            }
        }

        fn add(&self, id: &str, url: &str) {
            add_book(
                &self.layout,
                id,
                &format!("Book {id}"),
                url,
                Some("Fixture"),
            )
            .unwrap();
        }

        fn menu(&self, value: &str) {
            let path = self.layout.menu_path();
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, value).unwrap();
        }
    }

    #[derive(Clone)]
    struct FakeFetcher {
        document: FetchedDocument,
        calls: Cell<usize>,
    }

    impl FakeFetcher {
        fn html(value: &str) -> Self {
            Self {
                document: FetchedDocument {
                    final_url: "https://example.com/chapter".into(),
                    content_type: "text/html".into(),
                    bytes: value.as_bytes().to_vec(),
                },
                calls: Cell::new(0),
            }
        }
    }

    impl Fetcher for FakeFetcher {
        fn fetch(&self, _url: &Url) -> Result<FetchedDocument, String> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.document.clone())
        }
    }

    #[test]
    fn learn_books_round_trip_deterministically() {
        let fixture = Fixture::new();
        fixture.add("rust-book", "https://doc.rust-lang.org/book/");
        let first = fs::read(fixture.layout.registry_path()).unwrap();
        let report = books(&fixture.layout).unwrap();
        assert!(report.books.iter().any(|book| book.id == "rust-book"));
        assert_eq!(first, fs::read(fixture.layout.registry_path()).unwrap());
        remove_book(&fixture.layout, "rust-book").unwrap();
        assert!(
            !books(&fixture.layout)
                .unwrap()
                .books
                .iter()
                .any(|book| book.id == "rust-book")
        );
        assert!(
            fixture
                .layout
                .registry_path()
                .starts_with(&fixture.layout.config_root)
        );
    }

    #[test]
    fn learn_books_reject_invalid_ids_and_urls() {
        let fixture = Fixture::new();
        let before = fs::read(fixture.layout.registry_path()).ok();
        for (id, url) in [
            ("Bad", "https://example.com"),
            ("local", "http://example.com"),
            ("creds", "https://user:pass@example.com"),
            ("localhost", "https://localhost/book"),
            ("literal", "https://127.0.0.1/book"),
            ("secret", "https://example.com/?token=private"),
        ] {
            assert!(add_book(&fixture.layout, id, "Invalid", url, None).is_err());
        }
        assert_eq!(before, fs::read(fixture.layout.registry_path()).ok());
    }

    #[test]
    fn learn_menu_sync_preserves_foreign_jsonc() {
        let fixture = Fixture::new();
        fixture.add("rust-book", "https://doc.rust-lang.org/book/");
        fixture.menu("{\n  // keep this comment\n  \"personal.notes\": {\"label\":\"Notes\"}\n}\n");
        sync_menu(&fixture.layout).unwrap();
        let once = fs::read_to_string(fixture.layout.menu_path()).unwrap();
        sync_menu(&fixture.layout).unwrap();
        let twice = fs::read_to_string(fixture.layout.menu_path()).unwrap();
        assert_eq!(once, twice);
        assert!(twice.contains("// keep this comment"));
        assert!(twice.contains("personal.notes"));
        assert!(twice.contains("learn.book-rust-book"));
    }

    #[test]
    fn learn_menu_unsync_removes_only_owned_block() {
        let fixture = Fixture::new();
        fixture.menu("{\n  \"foreign\": {\"label\":\"Keep\"},\n}\n");
        sync_menu(&fixture.layout).unwrap();
        let path = fixture.layout.menu_path();
        let with_foreign_edit = fs::read_to_string(&path)
            .unwrap()
            .replace("\"Keep\"", "\"Still Keep\"");
        fs::write(&path, with_foreign_edit).unwrap();
        unsync_menu(&fixture.layout).unwrap();
        let result = fs::read_to_string(path).unwrap();
        assert!(result.contains("Still Keep"));
        assert!(!result.contains(MENU_BEGIN));
        assert!(!result.contains("agent-translate"));
    }

    #[test]
    fn learn_menu_rejects_corrupt_markers() {
        let fixture = Fixture::new();
        for value in [
            format!("{{\n  {MENU_BEGIN}\n}}\n"),
            format!("{{\n  {MENU_END}\n  {MENU_BEGIN}\n}}\n"),
            format!("{{\n  {MENU_BEGIN}\n  {MENU_BEGIN}\n  {MENU_END}\n}}\n"),
        ] {
            fixture.menu(&value);
            let before = fs::read(fixture.layout.menu_path()).unwrap();
            assert!(sync_menu(&fixture.layout).is_err());
            assert_eq!(before, fs::read(fixture.layout.menu_path()).unwrap());
        }
    }

    #[test]
    fn learn_plan_excludes_source_and_private_content() {
        let fixture = Fixture::new();
        fixture.add("private-book", "https://example.com/chapter");
        fs::write(
            fixture.layout.home.join("credential"),
            "CREDENTIAL_SENTINEL",
        )
        .unwrap();
        let plan = create_plan(&fixture.layout, "private-book", Agent::Codex, "zh-CN").unwrap();
        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("maximumSourceBytes"));
        assert!(!json.contains("SOURCE_BODY_SENTINEL"));
        assert!(!json.contains("CREDENTIAL_SENTINEL"));
    }

    #[test]
    fn learn_apply_rejects_wrong_confirmation() {
        let fixture = Fixture::new();
        let plan = create_plan(&fixture.layout, "omarchy", Agent::Codex, "zh-CN").unwrap();
        let fetcher = FakeFetcher::html("<p>source</p>");
        let invoked = Cell::new(false);
        let result = apply_with(&fixture.layout, &plan.id, "wrong", &fetcher, |_| {
            invoked.set(true);
            Ok("translated".into())
        });
        assert!(result.is_err());
        assert!(!invoked.get());
        assert!(!fixture.layout.translations_dir().exists());
    }

    #[test]
    fn learn_apply_rejects_changed_book() {
        let fixture = Fixture::new();
        fixture.add("changing", "https://example.com/one");
        let plan = create_plan(&fixture.layout, "changing", Agent::Codex, "zh-CN").unwrap();
        let mut registry = load_registry(&fixture.layout).unwrap();
        registry.books[0].url = "https://example.com/two".into();
        atomic_json(&fixture.layout.registry_path(), &registry).unwrap();
        let invoked = Cell::new(false);
        let result = apply_with(
            &fixture.layout,
            &plan.id,
            &plan.confirmation_token,
            &FakeFetcher::html("source"),
            |_| {
                invoked.set(true);
                Ok("translated".into())
            },
        );
        assert!(result.unwrap_err().contains("registry changed"));
        assert!(!invoked.get());
    }

    #[test]
    fn learn_fetch_rejects_private_redirect() {
        let fixture = Fixture::new();
        let plan = create_plan(&fixture.layout, "omarchy", Agent::Codex, "zh-CN").unwrap();
        let fetcher = FakeFetcher {
            document: FetchedDocument {
                final_url: "https://127.0.0.1/private".into(),
                content_type: "text/plain".into(),
                bytes: b"private".to_vec(),
            },
            calls: Cell::new(0),
        };
        let result = apply_with(
            &fixture.layout,
            &plan.id,
            &plan.confirmation_token,
            &fetcher,
            |_| Ok("translated".into()),
        );
        assert!(result.unwrap_err().contains("local or an IP literal"));
    }

    #[test]
    fn learn_fetch_accepts_active_mihomo_fake_dns_only() {
        let fake_v4 = "198.18.0.111".parse().unwrap();
        let fake_v6 = "fdfe:dcba:9876::66".parse().unwrap();
        let fake_answers = [fake_v4, fake_v6];
        assert!(resolved_addresses_are_safe(&fake_answers, true));
        assert!(!resolved_addresses_are_safe(&fake_answers, false));

        for unsafe_ip in [
            "127.0.0.1".parse().unwrap(),
            "192.168.1.10".parse().unwrap(),
            "fd00::1".parse().unwrap(),
        ] {
            assert!(!resolved_addresses_are_safe(&[fake_v4, unsafe_ip], true));
        }
    }

    #[test]
    fn learn_fetch_rejects_non_text_and_oversized_content() {
        let fixture = Fixture::new();
        for (kind, bytes) in [
            ("application/octet-stream", vec![0; 20]),
            ("text/plain", vec![b'x'; MAX_SOURCE_BYTES + 1]),
        ] {
            let plan = create_plan(&fixture.layout, "omarchy", Agent::Codex, "zh-CN").unwrap();
            let fetcher = FakeFetcher {
                document: FetchedDocument {
                    final_url: "https://example.com/chapter".into(),
                    content_type: kind.into(),
                    bytes,
                },
                calls: Cell::new(0),
            };
            assert!(
                apply_with(
                    &fixture.layout,
                    &plan.id,
                    &plan.confirmation_token,
                    &fetcher,
                    |_| Ok("translated".into())
                )
                .is_err()
            );
        }
    }

    #[test]
    fn learn_fetch_reads_exactly_one_document() {
        let fixture = Fixture::new();
        let plan = create_plan(&fixture.layout, "omarchy", Agent::Codex, "zh-CN").unwrap();
        let fetcher =
            FakeFetcher::html("<article>one<a href='https://example.com/two'>two</a></article>");
        apply_with(
            &fixture.layout,
            &plan.id,
            &plan.confirmation_token,
            &fetcher,
            |_| Ok("translated".into()),
        )
        .unwrap();
        assert_eq!(fetcher.calls.get(), 1);
    }

    #[test]
    fn learn_agent_adapters_use_safe_direct_argv() {
        let fixture = Fixture::new();
        for agent in [Agent::Codex, Agent::Claude, Agent::Grok] {
            let plan = create_plan(&fixture.layout, "omarchy", agent, "zh-CN").unwrap();
            let invocation = agent_invocation(&fixture.layout, &plan, "prompt".into()).unwrap();
            assert_eq!(invocation.executable, fixture.layout.agents[&agent]);
            let args = invocation.args.join(" ");
            match agent {
                Agent::Codex => {
                    assert!(args.contains("--ephemeral"));
                    assert!(args.contains("--sandbox read-only"));
                }
                Agent::Claude => {
                    assert!(args.contains("--no-session-persistence"));
                    assert!(args.contains("--tools "));
                }
                Agent::Grok => {
                    assert!(args.contains("--disable-web-search"));
                    assert!(args.contains("--max-turns 1"));
                    assert!(args.contains("--permission-mode plan"));
                    assert!(args.contains("--prompt-file /dev/stdin"));
                    assert!(args.contains("--output-format plain"));
                    assert!(!args.contains("--output-format text"));
                    assert!(invocation.stdin.is_some());
                }
                Agent::Octos => unreachable!(),
            }
            assert!(!args.contains("bash"));
            assert!(!args.contains("sudo"));
        }
    }

    #[test]
    fn learn_octos_translation_is_unavailable() {
        let fixture = Fixture::new();
        let error = create_plan(&fixture.layout, "omarchy", Agent::Octos, "zh-CN").unwrap_err();
        assert!(error.contains("octos-unavailable"));
        assert!(!fixture.layout.plans_dir().exists());
    }

    #[test]
    fn learn_agent_failure_is_atomic() {
        let fixture = Fixture::new();
        let plan = create_plan(&fixture.layout, "omarchy", Agent::Codex, "zh-CN").unwrap();
        let result = apply_with(
            &fixture.layout,
            &plan.id,
            &plan.confirmation_token,
            &FakeFetcher::html("<p>source</p>"),
            |_| Err("synthetic-agent-failure".into()),
        );
        assert!(result.unwrap_err().contains("synthetic-agent-failure"));
        assert!(!fixture.layout.translations_dir().exists());
    }

    #[test]
    fn learn_translation_is_escaped_attributed_and_cached() {
        let fixture = Fixture::new();
        let plan = create_plan(&fixture.layout, "omarchy", Agent::Codex, "zh-CN").unwrap();
        let fetcher = FakeFetcher::html("<article>SOURCE_BODY_SENTINEL</article>");
        let calls = Cell::new(0);
        let first = apply_with(
            &fixture.layout,
            &plan.id,
            &plan.confirmation_token,
            &fetcher,
            |_| {
                calls.set(calls.get() + 1);
                Ok("<script>alert('x')</script> translated".into())
            },
        )
        .unwrap();
        let html = fs::read_to_string(&first.output_path).unwrap();
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("Original source"));
        assert!(html.contains("Translated by codex"));
        let second = apply_with(
            &fixture.layout,
            &plan.id,
            &plan.confirmation_token,
            &fetcher,
            |_| {
                calls.set(calls.get() + 1);
                Ok("must-not-run".into())
            },
        )
        .unwrap();
        assert!(second.cached);
        assert_eq!(calls.get(), 1);
        let result_json = serde_json::to_string(&second).unwrap();
        assert!(!result_json.contains("SOURCE_BODY_SENTINEL"));
    }

    #[test]
    fn learn_translation_rejects_oversized_agent_output() {
        let fixture = Fixture::new();
        let plan = create_plan(&fixture.layout, "omarchy", Agent::Codex, "zh-CN").unwrap();
        let result = apply_with(
            &fixture.layout,
            &plan.id,
            &plan.confirmation_token,
            &FakeFetcher::html("source"),
            |_| Ok("x".repeat(MAX_AGENT_OUTPUT_BYTES + 1)),
        );
        assert!(result.unwrap_err().contains("exceeded 2 MiB"));
        assert!(!fixture.layout.translations_dir().exists());
    }
}
