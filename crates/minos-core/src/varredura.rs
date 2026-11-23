//! Varredura em memória antes de publicar (`src/lib/contrib-scan.ts`, `src/lib/release-guard.ts:1-77` e
//! `src/lib/minos-guard.ts:12-39`): credencial, caminho da máquina, e-mail, nome e domínio de projeto, mais os valores do
//! `credenciais.md`. O achado diz rótulo:linha e o tipo, nunca o trecho.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use hades_core::env::Env;
use hades_core::profile::{domain_terms, load_profile, name_terms, vocabulary};
use hades_core::project::resolve_project;
use regex::{Regex, RegexBuilder};

/// `SECRET_PARTS` (`release-guard.ts:10`): montados em partes, para o fonte não conter o literal procurado.
const SECRET_PARTS: [(&str, &str); 3] = [
    ("Hds!", "[A-Za-z0-9]"),
    ("eyJh", "bGci"),
    ("BEGIN (RSA|OPENSSH) ", "PRIVATE"),
];
/// `SECRET_TAIL` (`release-guard.ts:13`), com o `\b` ASCII do JS.
const SECRET_TAIL: &str = r"devtunnels\.ms/?[^ ]*[a-z0-9]{8}-|(?-u:\b)(JWT_SECRET|INTERNAL_SERVICE_TOKEN|MINIO_ROOT_PASSWORD)=[A-Za-z0-9_-]{8,}";
const SLUG_KEY: &str = "HADES_VERSION_CHECK_REPO";
/// `MIN_SECRET_LENGTH` (`minos-guard.ts:12`).
const MIN_SECRET_LENGTH: usize = 8;

fn fixo(p: &str) -> Regex {
    Regex::new(p).expect("padrão fixo")
}

/// `MACHINE_PATH`, `EMAIL`, `EMAIL_NEUTRAL` e `CREDENTIAL_PATTERNS` (`contrib-scan.ts:19-29`).
static MACHINE_PATH: LazyLock<Regex> = LazyLock::new(|| fixo(r"/(?:home|Users)/[A-Za-z0-9._-]+/"));
static EMAIL: LazyLock<Regex> =
    LazyLock::new(|| fixo(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}"));
static EMAIL_NEUTRAL: LazyLock<Regex> = LazyLock::new(|| {
    fixo(r"(?i-u)^(?:noreply|no-reply)@|@(?:example\.(?:com|org|net|invalid)|[a-z.]*invalid)$")
});
static CREDENTIAL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?-u:\b)gh[opsur]_[A-Za-z0-9]{30,}",
        r"(?-u:\b)AKIA[0-9A-Z]{16}(?-u:\b)",
        r"(?-u:\b)sk-[A-Za-z0-9_-]{20,}",
        r"(?-u:\b)xox[abprs]-[A-Za-z0-9-]{10,}",
        r"(?-u:\b)Bearer\s+[A-Za-z0-9._~+/=-]{20,}",
        r#"(?i)(?-u:\b)(?:password|passwd|senha|secret|token|api[_-]?key)(?-u:\b)["']?\s*[:=]\s*["'][^"'\s]{8,}["']"#,
    ]
    .into_iter()
    .map(fixo)
    .collect()
});
/// `CREDENTIAL_LINE` (`minos-guard.ts:13`).
static CREDENTIAL_LINE: LazyLock<Regex> =
    LazyLock::new(|| fixo(r"^\s*(?:[-*]\s*)?`?[A-Z][A-Z0-9_]*`?\s*[=:]\s*(.*?)\s*$"));

/// `GuardTerms` (`release-guard.ts:18`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuardTerms {
    pub names: Vec<String>,
    pub vocab: Vec<String>,
    pub domain: Vec<String>,
    pub allow: Vec<String>,
    pub slug: String,
}

/// O projeto ativo, dispensado da varredura de nome na página pessoal de documento.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjetoAtivo {
    pub name: String,
    pub vocab: Vec<String>,
}

/// `activeProjectTerms` (`release-guard.ts:32`): nome e vocabulário do perfil do `cwd`; `None` sem perfil.
pub fn active_project_terms(env: &Env, cwd: &Path) -> Option<ProjetoAtivo> {
    let resolution = resolve_project(env, cwd);
    let name = resolution
        .project
        .clone()
        .filter(|_| resolution.code == 0)?;
    let profile = load_profile(&resolution);
    let vocab = profile
        .get("identity")
        .and_then(|i| i.as_object())
        .and_then(|i| i.get("vocabulary"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    Some(ProjetoAtivo { name, vocab })
}

/// `guardTerms` (`release-guard.ts:44`): os termos dos perfis instalados, menos os do projeto `except`.
pub fn guard_terms(env: &Env, roots: &[PathBuf], except: Option<&ProjetoAtivo>) -> GuardTerms {
    let domain = domain_terms(env);
    let dropped: BTreeSet<&str> = except
        .map(|e| e.vocab.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let names: BTreeSet<String> = name_terms(env)
        .into_iter()
        .filter(|(name, _)| except.is_none_or(|e| &e.name != name))
        .flat_map(|(_, terms)| terms)
        .collect();
    GuardTerms {
        names: names.into_iter().collect(),
        vocab: vocabulary(env)
            .into_iter()
            .filter(|v| !v.is_empty() && !dropped.contains(v.as_str()))
            .collect(),
        domain: domain.terms,
        allow: domain.allow.into_iter().filter(|a| !a.is_empty()).collect(),
        slug: skill_repo_slug(roots),
    }
}

/// `skillRepoSlug` (`release-guard.ts:57`).
pub fn skill_repo_slug(roots: &[PathBuf]) -> String {
    for root in roots {
        let text = std::fs::read_to_string(root.join("kit").join("config.tsv")).unwrap_or_default();
        if let Some(row) = text
            .split('\n')
            .find(|l| l.split('\t').next() == Some(SLUG_KEY))
        {
            return row.split('\t').nth(1).unwrap_or_default().to_owned();
        }
    }
    String::new()
}

/// `hostname()` do Node.
fn os_hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|h| h.trim_end_matches(['\r', '\n']).to_owned())
        .unwrap_or_default()
}

/// `userInfo().username` do Node: o nome do uid efetivo no `/etc/passwd`; sem ele, `USER`.
fn os_username() -> String {
    let uid = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("Uid:"))
                .and_then(|r| r.split_whitespace().nth(1).map(str::to_owned))
        });
    let from_passwd = uid.and_then(|uid| {
        std::fs::read_to_string("/etc/passwd")
            .ok()?
            .lines()
            .find_map(|l| {
                let f: Vec<&str> = l.split(':').collect();
                (f.len() > 2 && f[2] == uid).then(|| f[0].to_owned())
            })
    });
    from_passwd.unwrap_or_else(|| std::env::var("USER").unwrap_or_default())
}

/// `identityPattern` (`release-guard.ts:71`): segredo e identidade da máquina (usuário e host desta máquina).
pub fn identity_pattern() -> String {
    let mut host = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .replace(['\r', '\n'], "");
    if host.is_empty() || host == "sandbox" || host == "localhost" {
        host = os_hostname();
    }
    if host == "sandbox" || host == "localhost" {
        host.clear();
    }
    let mut parts: Vec<String> = SECRET_PARTS
        .iter()
        .map(|(a, b)| format!("{a}{b}"))
        .collect();
    parts.push(format!("/home/{}/", os_username()));
    if !host.is_empty() {
        parts.push(host);
    }
    parts.push(SECRET_TAIL.to_owned());
    parts.join("|")
}

/// `Text` (`contrib-scan.ts:14`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texto {
    pub label: String,
    pub text: String,
}

/// `Finding` (`contrib-scan.ts:7`), só de texto (a linha começa em 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Achado {
    pub file: String,
    pub line: usize,
    pub kind: &'static str,
}

/// `formatFinding` (`contrib-scan.ts:92`).
pub fn format_finding(f: &Achado) -> String {
    format!("{}:{}  {}", f.file, f.line, f.kind)
}

/// `compile` (`contrib-scan.ts:31`). O texto do erro do motor difere do JS.
fn compile(source: &str, what: &str) -> Result<Regex, String> {
    RegexBuilder::new(source)
        .case_insensitive(true)
        .build()
        .map_err(|e| format!("termo de {what} não compila ({source}): {e}"))
}

struct Matchers {
    names: Vec<Regex>,
    domain: Vec<Regex>,
    identity: Regex,
    slug: String,
}

/// `matchers` (`contrib-scan.ts:48`); nomes e vocabulário juntos, como em `kindOf`.
fn matchers(terms: &GuardTerms) -> Result<Matchers, String> {
    let mut names = Vec::new();
    for t in &terms.names {
        names.push(compile(&regex::escape(t), "nome")?);
    }
    for t in &terms.vocab {
        names.push(compile(t, "vocabulário")?);
    }
    let domain = terms
        .domain
        .iter()
        .map(|t| compile(t, "domínio"))
        .collect::<Result<_, _>>()?;
    let identity = Regex::new(&identity_pattern())
        .map_err(|e| format!("termo de identidade não compila: {e}"))?;
    Ok(Matchers {
        names,
        domain,
        identity,
        slug: terms.slug.to_lowercase(),
    })
}

/// `kindOf` (`contrib-scan.ts:61`): o primeiro tipo que casa, na ordem segredo, caminho, e-mail, nome, domínio.
fn kind_of(line: &str, m: &Matchers) -> Option<&'static str> {
    if m.identity.is_match(line) || CREDENTIAL_PATTERNS.iter().any(|p| p.is_match(line)) {
        return Some("credencial");
    }
    if MACHINE_PATH.is_match(line) {
        return Some("caminho da máquina");
    }
    if EMAIL
        .find_iter(line)
        .any(|e| !EMAIL_NEUTRAL.is_match(e.as_str()))
    {
        return Some("e-mail");
    }
    let exempt = !m.slug.is_empty() && line.to_lowercase().contains(&m.slug);
    if !exempt && m.names.iter().any(|p| p.is_match(line)) {
        return Some("nome de projeto");
    }
    if m.domain.iter().any(|p| p.is_match(line)) {
        return Some("domínio de projeto");
    }
    None
}

/// `scanContribution` (`contrib-scan.ts:72`) sem núcleo: só os textos.
pub fn scan_texts(texts: &[Texto], terms: &GuardTerms) -> Result<Vec<Achado>, String> {
    let m = matchers(terms)?;
    let mut out = Vec::new();
    for t in texts {
        for (i, line) in t.text.split('\n').enumerate() {
            if let Some(kind) = kind_of(line, &m) {
                out.push(Achado {
                    file: t.label.clone(),
                    line: i + 1,
                    kind,
                });
            }
        }
    }
    Ok(out)
}

/// `credentialValues` (`minos-guard.ts:16`): valores `CHAVE=valor` com tamanho de segredo.
pub fn credential_values(credentials: Option<&str>) -> Vec<String> {
    let strip = |v: &str| -> String {
        let quote = |c: char| matches!(c, '"' | '\'' | '`');
        let v = v.strip_prefix(quote).unwrap_or(v);
        v.strip_suffix(quote).unwrap_or(v).to_owned()
    };
    credentials
        .unwrap_or_default()
        .split('\n')
        .filter_map(|line| CREDENTIAL_LINE.captures(line).map(|c| strip(&c[1])))
        .filter(|v| v.encode_utf16().count() >= MIN_SECRET_LENGTH)
        .collect()
}

/// `scanOutput` (`minos-guard.ts:30`): os motivos da recusa (vazio = pode sair).
pub fn scan_output(
    texts: &[Texto],
    credentials: Option<&str>,
    terms: &GuardTerms,
) -> Result<Vec<String>, String> {
    let mut reasons: Vec<String> = scan_texts(texts, terms)?
        .iter()
        .map(format_finding)
        .collect();
    let values = credential_values(credentials);
    for t in texts {
        for (i, line) in t.text.split('\n').enumerate() {
            if values.iter().any(|v| line.contains(v.as_str())) {
                reasons.push(format!("{}:{}  valor do credenciais.md", t.label, i + 1));
            }
        }
    }
    Ok(reasons)
}

#[cfg(test)]
#[path = "varredura_tests.rs"]
mod tests;
