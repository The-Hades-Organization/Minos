//! Coletor do modelo `orquestracao` (`src/lib/minos-collect.ts`): monta os dados do que a Hades já tem (plano, registro
//! dos agentes, perguntas abertas, % das janelas de uso). Fonte ausente vira lista vazia ou `null`, nunca número
//! inventado. [`build_orchestration`] é pura; a leitura do ambiente fica em [`collect`].

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use hades_core::config::config_value;
use hades_core::env::Env;
use hades_core::fsx::read_text;
use hades_core::limits::{self, Janela};
use hades_core::registry::{iso_from_epoch, open_question_heads};
use hades_core::work::{DEFAULT_REPORTS, Json, WorkEnv, agent_files, read_json};
use regex::Regex;
use serde_json::{Value, json};

use crate::check::{ContractError, Escopo, js_len};
use crate::modelo::{
    AGENT_STATES, Confianca, Estimativa, Limite, Limites, ORQUESTRACAO_SCHEMA, Orquestracao,
    Unidade, validar_orquestracao,
};

const TEXT_CAP: usize = 480;
const UNKNOWN_TITLE: &str = "Orquestração";
const MS: f64 = 1000.0;
const PCT_MAX: f64 = 100.0;

/// Espaço do `trim`/`\s` do JS (ECMA-262 WhiteSpace + LineTerminator).
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// `cap` (`minos-collect.ts:16`): acima de 480 unidades UTF-16, as 479 primeiras e `…`. Um par substituto cortado ao
/// meio sai inteiro de fora (o JS deixaria meia unidade).
pub fn cap(value: &str) -> String {
    if js_len(value) <= TEXT_CAP {
        return value.to_owned();
    }
    let mut units = 0;
    let mut out = String::new();
    for c in value.chars() {
        units += c.len_utf16();
        if units > TEXT_CAP - 1 {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

/// `bare` (`minos-collect.ts:17`).
fn bare(value: &str) -> String {
    js_trim(&value.replace('`', "")).to_owned()
}

/// `cells` (`minos-collect.ts:18`).
fn cells(row: &str) -> Vec<String> {
    let row = js_trim(row);
    let row = row.strip_prefix('|').unwrap_or(row);
    let row = row.strip_suffix('|').unwrap_or(row);
    row.split('|').map(|c| js_trim(c).to_owned()).collect()
}

/// `EMPTY_CELL` (`minos-collect.ts:19`).
fn empty_cell(cell: &str) -> bool {
    matches!(cell, "" | "—" | "-" | "–")
        || ["n/a", "nenhum", "nenhuma"]
            .iter()
            .any(|w| cell.eq_ignore_ascii_case(w))
}

/// `sectionLines` (`minos-collect.ts:22`): as linhas de `## <título>` até o próximo `## ` fora de bloco cercado.
pub fn section_lines(plan: &str, title: &str) -> Vec<String> {
    let text = plan.replace('\r', "");
    let lines: Vec<&str> = text.split('\n').collect();
    let head = format!("## {title}");
    let is_head = |l: &str| {
        l.strip_prefix(head.as_str()).is_some_and(|rest| {
            rest.chars()
                .next()
                .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_')
        })
    };
    let Some(start) = lines.iter().position(|l| is_head(l)) else {
        return Vec::new();
    };
    let mut body = Vec::new();
    let mut fenced = false;
    for line in &lines[start + 1..] {
        if line.trim_start_matches(is_js_space).starts_with("```") {
            fenced = !fenced;
        }
        if !fenced && line.starts_with("## ") {
            break;
        }
        body.push((*line).to_owned());
    }
    body
}

struct Tabela {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn is_rule(cell: &str) -> bool {
    let c = cell.strip_prefix(':').unwrap_or(cell);
    let c = c.strip_suffix(':').unwrap_or(c);
    c.len() >= 2 && c.bytes().all(|b| b == b'-')
}

/// `tableRows` (`minos-collect.ts:37`).
fn table_rows(lines: &[String]) -> Tabela {
    let mut table = lines
        .iter()
        .filter(|l| js_trim(l).starts_with('|'))
        .map(|l| cells(l))
        .filter(|r| !r.iter().all(|c| is_rule(c)));
    let header = table
        .next()
        .unwrap_or_default()
        .iter()
        .map(|c| c.to_lowercase())
        .collect();
    Tabela {
        header,
        rows: table.collect(),
    }
}

/// `.` do JS: qualquer caractere menos terminador de linha.
static COLUNAS: LazyLock<[Regex; 6]> = LazyLock::new(|| {
    [
        r"^unidade",
        r"^depende",
        r"^papel",
        r"^confian",
        r"^custo mediana",
        r"^dura[^\n\r\u{2028}\u{2029}]{2}o mediana",
    ]
    .map(|p| Regex::new(p).expect("padrão fixo"))
});

#[derive(Clone, Copy)]
enum Coluna {
    Unidade,
    Depende,
    Papel,
    Confianca,
    Custo,
    Duracao,
}

/// `column` (`minos-collect.ts:43`).
fn column<'a>(header: &[String], row: &'a [String], col: Coluna) -> &'a str {
    let re = &COLUNAS[col as usize];
    header
        .iter()
        .position(|h| re.is_match(h))
        .and_then(|i| row.get(i))
        .map_or("", String::as_str)
}

static DECIMAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[+-]?(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$").expect("padrão fixo")
});

/// `Number(s)` do JS (ECMA-262 §7.1.4.1.1). `None` é `NaN`.
fn js_number(text: &str) -> Option<f64> {
    let s = js_trim(text);
    if s.is_empty() {
        return Some(0.0);
    }
    for (prefix, radix) in [
        ("0x", 16),
        ("0X", 16),
        ("0o", 8),
        ("0O", 8),
        ("0b", 2),
        ("0B", 2),
    ] {
        if let Some(digits) = s.strip_prefix(prefix) {
            if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
                return None;
            }
            return Some(digits.chars().fold(0.0, |n, c| {
                n * f64::from(radix) + f64::from(c.to_digit(radix).unwrap_or(0))
            }));
        }
    }
    match s {
        "Infinity" | "+Infinity" => return Some(f64::INFINITY),
        "-Infinity" => return Some(f64::NEG_INFINITY),
        _ => {}
    }
    if !DECIMAL.is_match(s) {
        return None;
    }
    s.parse().ok()
}

/// `numberOf` (`minos-collect.ts:47`): número finito e não negativo; a primeira vírgula vale ponto.
fn number_of(cell: &str) -> Option<f64> {
    if js_trim(cell).is_empty() {
        return None;
    }
    let n = js_number(js_trim(&cell.replacen(',', ".", 1)))?;
    (n.is_finite() && n >= 0.0).then_some(n)
}

/// `/^#\s+(.+)$/m` (`minos-collect.ts:53`) sobre o texto do plano, com o `^`/`$` multilinha e o `\s` do JS.
fn heading(plan: &str) -> Option<&str> {
    let mut starts = vec![0];
    starts.extend(
        plan.char_indices()
            .filter(|(_, c)| is_line_terminator(*c))
            .map(|(i, c)| i + c.len_utf8()),
    );
    starts.into_iter().find_map(|start| {
        let rest = plan[start..].strip_prefix('#')?;
        let ws_end = rest
            .char_indices()
            .find(|(_, c)| !is_js_space(*c))
            .map_or(rest.len(), |(i, _)| i);
        let first = rest.chars().next().filter(|c| is_js_space(*c))?;
        let after = &rest[ws_end..];
        let line_end = after.find(is_line_terminator).unwrap_or(after.len());
        if line_end > 0 {
            return Some(&after[..line_end]);
        }
        // fim do texto: o `\s+` devolve espaço para o `.+`, que só casa espaço fora de terminador
        let tail = &rest[first.len_utf8()..ws_end];
        tail.chars().any(|c| !is_line_terminator(c)).then_some("")
    })
}

/// `/^Plano\s*[—–-]\s*/i` (`minos-collect.ts:53`).
fn strip_plano(title: &str) -> &str {
    if !title
        .get(..5)
        .is_some_and(|h| h.eq_ignore_ascii_case("plano"))
    {
        return title;
    }
    let rest = title[5..].trim_start_matches(is_js_space);
    match rest.chars().next() {
        Some(c @ ('—' | '–' | '-')) => rest[c.len_utf8()..].trim_start_matches(is_js_space),
        _ => title,
    }
}

static UNIT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9-]{0,30}$").expect("padrão fixo"));

/// `ParsedPlan` (`minos-collect.ts:45`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlanoLido {
    pub title: String,
    pub objective: String,
    pub state: Vec<String>,
    pub units: Vec<Unidade>,
    pub estimate: Vec<Estimativa>,
}

/// `parsePlan` (`minos-collect.ts:52`): título sem o prefixo `Plano —`, objetivo, itens do estado atual e as tabelas
/// de unidades e de estimativa.
pub fn parse_plan(plan: &str) -> PlanoLido {
    let title = heading(plan).map(|t| js_trim(strip_plano(t))).unwrap_or("");
    let title = if title.is_empty() {
        UNKNOWN_TITLE
    } else {
        title
    };
    let objective = section_lines(plan, "Objetivo")
        .into_iter()
        .filter(|l| !js_trim(l).is_empty() && !l.starts_with("<!--"))
        .collect::<Vec<_>>()
        .join(" ");
    let state = section_lines(plan, "Estado atual")
        .iter()
        .map(|l| js_trim(l))
        .filter_map(|l| l.strip_prefix("- "))
        .map(|l| cap(&l.replace("**", "")))
        .collect();
    let Tabela { header, rows } = table_rows(&section_lines(plan, "Unidades"));
    let units = rows
        .iter()
        .filter_map(|row| {
            let name = bare(column(&header, row, Coluna::Unidade));
            if !UNIT_NAME.is_match(&name) {
                return None;
            }
            let depends_on = column(&header, row, Coluna::Depende)
                .split([',', ';'])
                .map(bare)
                .filter(|d| !empty_cell(d))
                .collect();
            Some(Unidade {
                name,
                role: cap(&bare(column(&header, row, Coluna::Papel))),
                depends_on,
            })
        })
        .collect();
    let Tabela { header, rows } = table_rows(&section_lines(plan, "Estimativa"));
    let estimate = rows
        .iter()
        .filter_map(|row| {
            let unit = bare(column(&header, row, Coluna::Unidade));
            if unit.is_empty() {
                return None;
            }
            let confidence = bare(column(&header, row, Coluna::Confianca)).to_lowercase();
            Some(Estimativa {
                unit,
                cost_p50: number_of(column(&header, row, Coluna::Custo)),
                minutes_p50: number_of(column(&header, row, Coluna::Duracao)),
                confidence: Confianca::parse(&confidence).unwrap_or(Confianca::Nenhuma),
            })
        })
        .collect();
    PlanoLido {
        title: cap(title),
        objective: cap(js_trim(&objective)),
        state,
        units,
        estimate,
    }
}

/// `AgentSource` (`minos-collect.ts:80`): um agente como o registro e os relatórios o descrevem.
#[derive(Debug, Clone, PartialEq)]
pub struct FonteAgente {
    pub name: String,
    pub mode: String,
    pub state: String,
    pub phase: String,
    pub branch: String,
    pub pr: Option<u64>,
    pub open_questions: Vec<String>,
    pub report_done: bool,
}

/// `OrchestrationInputs` (`minos-collect.ts:84`). `now_ms` é o instante em ms desde a época.
#[derive(Debug, Clone)]
pub struct Entradas {
    pub plan_text: String,
    pub agents: Vec<FonteAgente>,
    pub limits: Limites,
    pub scope: Escopo,
    pub now_ms: i64,
}

fn limite_value(l: &Option<Limite>) -> Value {
    l.as_ref().map_or(
        Value::Null,
        |l| json!({ "pct": l.pct, "resetsAt": l.resets_at }),
    )
}

/// `buildOrchestration` (`minos-collect.ts:94`): pura. O resultado passa de novo pelo contrato, como no TS: o que o
/// coletor monta tem de valer para quem lê.
pub fn build_orchestration(input: &Entradas) -> Result<Orquestracao, ContractError> {
    let plan = parse_plan(&input.plan_text);
    let agents: Vec<Value> = input
        .agents
        .iter()
        .map(|a| {
            let state = if AGENT_STATES.contains(&a.state.as_str()) {
                a.state.as_str()
            } else {
                "unknown"
            };
            json!({
                "name": a.name, "mode": a.mode, "state": state, "phase": cap(&a.phase), "branch": cap(&a.branch),
                "pr": a.pr, "openQuestions": a.open_questions.len(), "reportDone": a.report_done,
            })
        })
        .collect();
    let mut seen = Vec::new();
    let prs: Vec<Value> = input
        .agents
        .iter()
        .filter_map(|a| {
            let pr = a.pr.filter(|pr| !seen.contains(pr))?;
            seen.push(pr);
            Some(json!({ "number": pr, "branch": cap(&a.branch), "agent": a.name }))
        })
        .collect();
    let escalations: Vec<Value> = input
        .agents
        .iter()
        .flat_map(|a| {
            a.open_questions
                .iter()
                .map(|q| json!({ "agent": a.name, "question": cap(q) }))
        })
        .collect();
    let units: Vec<Value> = plan
        .units
        .iter()
        .map(|u| json!({ "name": u.name, "role": u.role, "dependsOn": u.depends_on }))
        .collect();
    let estimate: Vec<Value> = plan
        .estimate
        .iter()
        .map(|e| {
            json!({
                "unit": e.unit, "costP50": e.cost_p50, "minutesP50": e.minutes_p50,
                "confidence": e.confidence.as_str(),
            })
        })
        .collect();
    let data = json!({
        "model": "orquestracao",
        "schema": ORQUESTRACAO_SCHEMA,
        "scope": input.scope.as_str(),
        "generatedAt": iso_from_epoch(input.now_ms.div_euclid(1000)),
        "plan": { "title": plan.title, "objective": plan.objective, "state": plan.state, "units": units },
        "agents": agents,
        "prs": prs,
        "escalations": escalations,
        "consumption": {
            "estimate": estimate,
            "limits": { "five": limite_value(&input.limits.five), "seven": limite_value(&input.limits.seven) },
        },
    });
    validar_orquestracao(&data)
}

fn str_of(record: &Json, key: &str) -> String {
    record
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

/// `prNumber` (`minos-collect.ts:116`): inteiro positivo.
fn pr_number(value: Option<&Value>) -> Option<u64> {
    let n = value?.as_f64()?;
    (n.is_finite() && n.fract() == 0.0 && n > 0.0).then_some(n as u64)
}

/// `agentSource` (`minos-collect.ts:119`): o registro mais `questions.md` e `report.md` da pasta de relatórios.
fn agent_source(record: &Json, reports_dir: &str) -> FonteAgente {
    let worktree = str_of(record, "worktree");
    let worktree = if worktree.is_empty() {
        "/nonexistent".to_owned()
    } else {
        worktree
    };
    let w = Path::new(&worktree).join(reports_dir);
    let questions = read_text(&w.join("questions.md")).unwrap_or_default();
    let report = read_text(&w.join("report.md")).unwrap_or_default();
    FonteAgente {
        name: str_of(record, "name"),
        mode: str_of(record, "mode"),
        state: str_of(record, "state"),
        phase: str_of(record, "phase"),
        branch: str_of(record, "branch"),
        pr: pr_number(record.get("pr")),
        open_questions: open_question_heads(&questions),
        report_done: report
            .trim_start_matches(is_js_space)
            .starts_with("STATUS: DONE"),
    }
}

/// `janelaAtual` (`src/lib/minos-limits.ts:8`): a última leitura da janela dentro da retenção, no formato do contrato;
/// sem amostra, `None`. Lê a retenção a cada chamada, como o `chaves()` do TS (aviso no stderr se inválida).
pub fn janela_atual(janela: Janela, now_ms: i64, env: &Env, cwd: &Path) -> Option<Limite> {
    let dias = limits::retention_days(env, cwd);
    let amostras = limits::dentro_da_retencao(&limits::ler_amostras(env), now_ms as f64, dias);
    let a = limits::atuais(&amostras, now_ms as f64)
        .get(janela)?
        .clone();
    if !a.pct.is_finite() {
        return None;
    }
    Some(Limite {
        pct: a.pct.clamp(0.0, PCT_MAX),
        // o `Date` trunca para ms e o ISO corta os ms
        resets_at: iso_from_epoch(((a.resets_at * MS).trunc() / MS).floor() as i64),
    })
}

/// Recusa do coletor: plano ausente ou dados fora do contrato.
#[derive(Debug, Clone, PartialEq)]
pub enum ErroColeta {
    PlanoAusente(PathBuf),
    Contrato(ContractError),
}

impl fmt::Display for ErroColeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlanoAusente(p) => write!(f, "plano não encontrado: {}", p.display()),
            Self::Contrato(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ErroColeta {}

/// `collectOrchestration` (`minos-collect.ts:132`): lê o plano, os agentes do registro (`work`), a pasta de relatórios
/// (`HADES_ORCH_REPORTS_DIR`) e as janelas de uso, e monta os dados.
pub fn collect(
    plan_path: &Path,
    scope: Escopo,
    now_ms: i64,
    work: &WorkEnv,
    env: &Env,
    cwd: &Path,
) -> Result<Orquestracao, ErroColeta> {
    let plan_text =
        read_text(plan_path).ok_or_else(|| ErroColeta::PlanoAusente(plan_path.to_owned()))?;
    let reports_dir = config_value(env, cwd, "HADES_ORCH_REPORTS_DIR", DEFAULT_REPORTS);
    let agents = agent_files(work)
        .iter()
        .filter_map(|file| read_json(file))
        .filter(|record| !str_of(record, "name").is_empty())
        .map(|record| agent_source(&record, &reports_dir))
        .collect();
    let limits = Limites {
        five: janela_atual(Janela::Five, now_ms, env, cwd),
        seven: janela_atual(Janela::Seven, now_ms, env, cwd),
    };
    build_orchestration(&Entradas {
        plan_text,
        agents,
        limits,
        scope,
        now_ms,
    })
    .map_err(ErroColeta::Contrato)
}

#[cfg(test)]
#[path = "coleta_tests.rs"]
pub(crate) mod tests;
