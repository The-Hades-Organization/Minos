//! Página do modelo `orquestracao` (`src/lib/minos-html.ts`, `src/lib/minos-esc.ts`). O modelo
//! (`kit/minos/pagina.html`) é estático e o mesmo para todos; o corpo sai dos dados já validados e todo texto passa por
//! [`esc`]. Sem script além do bloco de dados e sem recurso externo.

use std::io;
use std::path::PathBuf;
use std::sync::LazyLock;

use hades_core::env::Env;
use hades_core::jsfmt::{to_fixed, to_js_string};
use hades_core::jsonfmt::json_compact;
use hades_core::paths::skill_root;
use regex::{Captures, Regex};

use crate::check::Escopo;
use crate::modelo::{Confianca, EstadoAgente, Limite, Modelo, Orquestracao, Validado};

const NO_VALUE: &str = "—";
const COST_DECIMALS: usize = 2;

/// `esc` (`src/lib/minos-esc.ts:3`): os cinco caracteres de marcação.
pub fn esc(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

static CRASE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`\n]+)`").expect("padrão fixo"));

/// `inline` (`src/lib/minos-doc-html.ts:25`): escapa e depois troca `` `x` `` por `<code>x</code>`.
pub fn inline(value: &str) -> String {
    CRASE
        .replace_all(&esc(value), "<code>$1</code>")
        .into_owned()
}

/// `Math.round` do JS: empate vai para +∞.
pub fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// `STATE_LABEL` (`minos-html.ts:17`): rótulo e classe do ponto.
fn state_label(state: EstadoAgente) -> (&'static str, &'static str) {
    match state {
        EstadoAgente::Working => ("trabalhando", "busy"),
        EstadoAgente::Idle => ("ocioso", "ok"),
        EstadoAgente::Blocked => ("espera por você", "warn"),
        EstadoAgente::Unknown => ("sem sessão", ""),
        EstadoAgente::Done => ("concluído", "ok"),
        EstadoAgente::Archived => ("arquivado", ""),
    }
}

/// `CONFIDENCE_LABEL` (`minos-html.ts:21`).
fn confidence_label(c: Confianca) -> &'static str {
    match c {
        Confianca::Alta => "alta",
        Confianca::Media => "média",
        Confianca::Baixa => "baixa",
        Confianca::Nenhuma => "sem histórico",
    }
}

/// `pageTitle` (`minos-html.ts:27`).
pub fn page_title(data: &Orquestracao) -> &'static str {
    match data.scope {
        Escopo::Pessoal => "Orquestração Hades",
        Escopo::Time => "Orquestração do time",
    }
}

/// `activeCount` (`minos-html.ts:29`): agentes fora de `done` e `archived`.
fn active_count(data: &Orquestracao) -> usize {
    data.agents
        .iter()
        .filter(|a| !matches!(a.state, EstadoAgente::Done | EstadoAgente::Archived))
        .count()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// `utcText` (`minos-html.ts:31`): `AAAA-MM-DD HH:MMZ`.
fn utc_text(iso: &str) -> String {
    let part = |a: usize, b: usize| iso.chars().skip(a).take(b - a).collect::<String>();
    format!("{} {}Z", part(0, 10), part(11, 16))
}

/// `pageDescription` (`minos-html.ts:33`).
pub fn page_description(data: &Orquestracao) -> String {
    format!(
        "Acompanhamento de {}: {}, {}, {}.",
        data.plan.title,
        plural(active_count(data), "agente ativo", "agentes ativos"),
        plural(
            data.escalations.len(),
            "pergunta aberta",
            "perguntas abertas"
        ),
        plural(data.prs.len(), "PR", "PRs"),
    )
}

/// `loadTemplate` (`minos-html.ts:38`): `<skill>/kit/minos/pagina.html`.
pub fn load_template(env: &Env) -> io::Result<String> {
    std::fs::read_to_string(template_path(env))
}

pub fn template_path(env: &Env) -> PathBuf {
    skill_root(env)
        .join("kit")
        .join("minos")
        .join("pagina.html")
}

/// `section` (`minos-html.ts:42`).
fn section(title: &str, summary: &str, inner: &str) -> String {
    format!(
        "<section><header><h2>{}</h2><span class=\"muted\">{}</span></header>{inner}</section>",
        esc(title),
        esc(summary)
    )
}

fn empty(message: &str) -> String {
    format!("<p class=\"muted\">{}</p>", esc(message))
}

fn table(head: &[&str], rows: &[String]) -> String {
    let head: String = head
        .iter()
        .map(|h| format!("<th>{}</th>", esc(h)))
        .collect();
    format!(
        "<div class=\"tablewrap\"><table><tr>{head}</tr>{}</table></div>",
        rows.concat()
    )
}

/// `escalations` (`minos-html.ts:49`).
fn escalations(data: &Orquestracao) -> String {
    let items: Vec<String> = data
        .escalations
        .iter()
        .map(|e| {
            format!(
                "<li class=\"row\"><span class=\"mono muted\">{}</span><span class=\"grow\">{}</span></li>",
                esc(&e.agent),
                esc(&e.question)
            )
        })
        .collect();
    let inner = if items.is_empty() {
        empty("Nenhuma pergunta aberta.")
    } else {
        format!("<ul>{}</ul>", items.concat())
    };
    section(
        "Perguntas ao dono",
        &plural(items.len(), "aberta", "abertas"),
        &inner,
    )
}

/// `agents` (`minos-html.ts:54`).
fn agents(data: &Orquestracao) -> String {
    let items: Vec<String> = data
        .agents
        .iter()
        .map(|a| {
            let (label, kind) = state_label(a.state);
            let pr = a.pr.map(|n| format!("#{n}")).unwrap_or_default();
            let extra = [a.phase.as_str(), a.branch.as_str(), pr.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            let questions = if a.open_questions > 0 {
                format!(
                    "<span class=\"warn-text\">{}</span>",
                    esc(&plural(
                        a.open_questions as usize,
                        "pergunta aberta",
                        "perguntas abertas"
                    ))
                )
            } else {
                String::new()
            };
            let done = if a.report_done {
                "<span>relatório pronto</span>".to_owned()
            } else {
                String::new()
            };
            let flags = [questions, done]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            let flags = if flags.is_empty() {
                flags
            } else {
                format!("<span>{flags}</span>")
            };
            format!(
                "<li class=\"row\"><strong>{}</strong><span class=\"muted\">{}</span><span><span class=\"dot {kind}\"></span>{}</span><span class=\"grow mono muted\">{}</span>{flags}</li>",
                esc(&a.name),
                esc(&a.mode),
                esc(label),
                esc(&extra)
            )
        })
        .collect();
    let inner = if items.is_empty() {
        empty("Nenhum agente registrado.")
    } else {
        format!("<ul>{}</ul>", items.concat())
    };
    section("Agentes", &plural(items.len(), "agente", "agentes"), &inner)
}

/// `prs` (`minos-html.ts:66`).
fn prs(data: &Orquestracao) -> String {
    let rows: Vec<String> = data
        .prs
        .iter()
        .map(|p| {
            format!(
                "<tr><td class=\"num\">#{}</td><td class=\"mono\">{}</td><td>{}</td></tr>",
                p.number,
                esc(&p.branch),
                esc(&p.agent)
            )
        })
        .collect();
    let inner = if rows.is_empty() {
        empty("Nenhuma PR aberta.")
    } else {
        table(&["PR", "Branch", "Agente"], &rows)
    };
    section("PRs", &plural(rows.len(), "PR", "PRs"), &inner)
}

/// `plan` (`minos-html.ts:71`).
fn plan(data: &Orquestracao) -> String {
    let state = if data.plan.state.is_empty() {
        empty("Plano sem \"Estado atual\".")
    } else {
        let items: String = data
            .plan
            .state
            .iter()
            .map(|s| format!("<li>{}</li>", esc(s)))
            .collect();
        format!("<ul>{items}</ul>")
    };
    let rows: Vec<String> = data
        .plan
        .units
        .iter()
        .map(|u| {
            let deps = u.depends_on.join(", ");
            let deps = if deps.is_empty() { NO_VALUE } else { &deps };
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&u.name),
                esc(&u.role),
                esc(deps)
            )
        })
        .collect();
    let units = if rows.is_empty() {
        String::new()
    } else {
        table(&["Unidade", "Papel", "Depende de"], &rows)
    };
    let objective = if data.plan.objective.is_empty() {
        String::new()
    } else {
        format!("<p>{}</p>", esc(&data.plan.objective))
    };
    section(
        "Plano",
        &plural(rows.len(), "unidade", "unidades"),
        &format!("{objective}{state}{units}"),
    )
}

/// `meter` (`minos-html.ts:78`).
fn meter(label: &str, limit: Option<&Limite>) -> String {
    let Some(limit) = limit else {
        return format!(
            "<div><div class=\"row\"><strong>{}</strong><span class=\"muted\">sem amostras</span></div></div>",
            esc(label)
        );
    };
    let pct = to_js_string(js_round(limit.pct));
    format!(
        "<div><div class=\"row\"><strong>{0}</strong><span class=\"mono\">{pct}%</span><span class=\"muted\">renova {1}</span></div><div class=\"meter\" role=\"img\" aria-label=\"{0}: {pct}% usado\"><span style=\"width:{pct}%\"></span></div></div>",
        esc(label),
        esc(&utc_text(&limit.resets_at))
    )
}

/// `consumption` (`minos-html.ts:85`).
fn consumption(data: &Orquestracao) -> String {
    let rows: Vec<String> = data
        .consumption
        .estimate
        .iter()
        .map(|e| {
            let cost = e.cost_p50.map_or(NO_VALUE.to_owned(), |c| {
                format!("US$ {}", to_fixed(c, COST_DECIMALS))
            });
            let minutes = e.minutes_p50.map_or(NO_VALUE.to_owned(), |m| {
                format!("{} min", to_js_string(js_round(m)))
            });
            format!(
                "<tr><td>{}</td><td class=\"num\">{cost}</td><td class=\"num\">{minutes}</td><td>{}</td></tr>",
                esc(&e.unit),
                esc(confidence_label(e.confidence))
            )
        })
        .collect();
    let estimates = if rows.is_empty() {
        empty("Plano sem estimativa da Moirai.")
    } else {
        table(
            &[
                "Unidade",
                "Custo (mediana)",
                "Duração (mediana)",
                "Confiança",
            ],
            &rows,
        )
    };
    let limits = &data.consumption.limits;
    section(
        "Consumo",
        "Moirai e janelas de uso",
        &format!(
            "{}{}{estimates}",
            meter("Janela de 5 h", limits.five.as_ref()),
            meter("Janela semanal", limits.seven.as_ref())
        ),
    )
}

/// `renderOrchestrationBody` (`minos-html.ts:93`).
pub fn render_orchestration_body(data: &Orquestracao) -> String {
    let lead = if data.escalations.is_empty() {
        "nenhuma pergunta aberta".to_owned()
    } else {
        plural(
            data.escalations.len(),
            "pergunta aberta",
            "perguntas abertas",
        )
    };
    let header = format!(
        "<header><h1>{}</h1><div class=\"meta\">{} · atualizado em {}</div><p class=\"lead\">{} · {} · {}</p></header>",
        esc(&data.plan.title),
        esc(page_title(data)),
        esc(&utc_text(&data.generated_at)),
        esc(&lead),
        esc(&plural(
            active_count(data),
            "agente ativo",
            "agentes ativos"
        )),
        esc(&plural(data.prs.len(), "PR", "PRs"))
    );
    [
        header,
        escalations(data),
        agents(data),
        prs(data),
        plan(data),
        consumption(data),
    ]
    .join("\n  ")
}

/// `dataBlock` (`minos-html.ts:102`): o JSON compacto dos dados para dentro de `<script>`; `<` vira `<` e
/// U+2028/U+2029 viram escape, então nenhum dado fecha o bloco nem abre outro.
pub fn data_block(data: &Validado) -> String {
    let mut out = String::new();
    for c in json_compact(&data.to_value()).chars() {
        match c {
            '<' => out.push_str("\\u003c"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            _ => out.push(c),
        }
    }
    out
}

/// `titleOf` (`minos-html.ts:105`). Só `orquestracao` por ora; o documento entra com o corpo dele.
pub fn title_of(data: &Orquestracao) -> &'static str {
    page_title(data)
}

/// `descriptionOf` (`minos-html.ts:110`).
pub fn description_of(data: &Orquestracao) -> String {
    page_description(data)
}

static MARCADOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{([A-Z]+)\}\}").expect("padrão fixo"));

/// `renderPage` (`minos-html.ts:120`): uma passada só sobre o modelo; o valor inserido nunca é relido como marcador e
/// marcador desconhecido fica como está.
pub fn render_page(template: &str, data: &Orquestracao) -> String {
    let layout = if Modelo::Orquestracao == data.model {
        "ops"
    } else {
        "doc"
    };
    let title = esc(title_of(data));
    let description = esc(&description_of(data));
    let body = render_orchestration_body(data);
    let block = data_block(&Validado::Orquestracao(data.clone()));
    MARCADOR
        .replace_all(template, |c: &Captures| {
            match &c[1] {
                "LAYOUT" => layout,
                "TITLE" => &title,
                "DESCRIPTION" => &description,
                "BODY" => &body,
                "DATA" => &block,
                _ => &c[0],
            }
            .to_owned()
        })
        .into_owned()
}

#[cfg(test)]
#[path = "pagina_tests.rs"]
mod tests;
