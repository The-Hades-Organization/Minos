//! Corpo das páginas `explicacao` e `proposta` (`src/lib/minos-doc-html.ts`). Todo texto passa por [`esc`]; a única
//! marcação aceita no texto é a crase, aplicada depois do escape por [`inline`].

use crate::check::Escopo;
use crate::documento::{
    Explicacao, Fonte, Nivel, Opcao, Passo, Proposta, StatusPergunta, StatusProposta, TipoProposta,
    Trecho,
};
use crate::pagina::{esc, inline, plural, utc_text};

const NO_VALUE: &str = "—";

/// `EXPLANATION_TITLE` (`minos-doc-html.ts:12`).
pub const EXPLANATION_TITLE: &str = "Explicação de código";

/// `TITLES` e `proposalTitle` (`minos-doc-html.ts:8`, `:19`).
pub fn proposal_title(data: &Proposta) -> &'static str {
    match data.kind {
        TipoProposta::Implementacao => "Proposta de implementação",
        TipoProposta::Correcao => "Proposta de correção",
        TipoProposta::Fluxo => "Proposta de fluxo",
        TipoProposta::Refatoracao => "Proposta de refatoração",
        TipoProposta::Discussao => "Discussão técnica",
    }
}

/// `STATUS_LABEL` (`minos-doc-html.ts:13`).
fn status_label(s: StatusProposta) -> (&'static str, &'static str) {
    match s {
        StatusProposta::Rascunho => ("rascunho", ""),
        StatusProposta::EmDiscussao => ("em discussão", "warn"),
        StatusProposta::Decidida => ("decidida", "ok"),
        StatusProposta::Descartada => ("descartada", "bad"),
    }
}

/// `LEVEL_LABEL` (`minos-doc-html.ts:16`).
fn level_label(l: Nivel) -> (&'static str, &'static str) {
    match l {
        Nivel::Baixo => ("baixo", "ok"),
        Nivel::Medio => ("médio", "warn"),
        Nivel::Alto => ("alto", "bad"),
    }
}

/// `scopeLabel` (`minos-doc-html.ts:22`).
fn scope_label(scope: Escopo) -> &'static str {
    match scope {
        Escopo::Time => "página do time",
        Escopo::Pessoal => "página pessoal",
    }
}

/// `section` (`minos-doc-html.ts:27`): o resumo só entra se não for vazio.
fn section(title: &str, summary: &str, inner: &str) -> String {
    let summary = if summary.is_empty() {
        String::new()
    } else {
        format!("<span class=\"muted\">{}</span>", esc(summary))
    };
    format!(
        "<section><header><h2>{}</h2>{summary}</header>{inner}</section>",
        esc(title)
    )
}

/// `prose` (`minos-doc-html.ts:31`).
fn prose(items: &[String]) -> String {
    let ps: String = items
        .iter()
        .map(|p| format!("<p>{}</p>", inline(p)))
        .collect();
    format!("<div class=\"prose\">{ps}</div>")
}

/// `bullets` (`minos-doc-html.ts:32`).
fn bullets(items: &[String]) -> String {
    let lis: String = items
        .iter()
        .map(|p| format!("<li>{}</li>", inline(p)))
        .collect();
    format!("<ul class=\"bullets\">{lis}</ul>")
}

/// `badge` (`minos-doc-html.ts:33`).
fn badge(label: &str, kind: &str) -> String {
    let kind = if kind.is_empty() {
        String::new()
    } else {
        format!(" {kind}")
    };
    format!("<span class=\"badge{kind}\">{}</span>", esc(label))
}

/// `levelBadge` (`minos-doc-html.ts:34`).
fn level_badge(name: &str, level: Option<Nivel>) -> String {
    level.map_or_else(String::new, |l| {
        let (label, kind) = level_label(l);
        badge(&format!("{name} {label}"), kind)
    })
}

fn non_empty(v: &Option<String>) -> Option<&str> {
    v.as_deref().filter(|s| !s.is_empty())
}

/// `snippets` (`minos-doc-html.ts:36`).
fn snippets(items: &[Trecho]) -> String {
    items
        .iter()
        .map(|c| {
            let where_: Vec<&str> = [Some(c.file.as_str()).filter(|s| !s.is_empty()), non_empty(&c.lines)]
                .into_iter()
                .flatten()
                .collect();
            let lang = non_empty(&c.lang).map_or_else(String::new, |l| format!(" · {}", esc(l)));
            let note = non_empty(&c.note).map_or_else(String::new, |n| format!(" · {}", inline(n)));
            format!(
                "<figure><figcaption>{}{lang}{note}</figcaption><pre><code>{}</code></pre></figure>",
                esc(&where_.join(":")),
                esc(&c.snippet)
            )
        })
        .collect()
}

/// `stepList` (`minos-doc-html.ts:44`).
fn step_list(items: &[Passo]) -> String {
    let lis: String = items
        .iter()
        .map(|s| {
            let where_ = non_empty(&s.r#where).map_or_else(String::new, |w| {
                format!("<span class=\"mono muted\">{}</span>", esc(w))
            });
            let detail = non_empty(&s.detail).map_or_else(String::new, |d| {
                format!("<span class=\"muted\">{}</span>", inline(d))
            });
            format!("<li><span>{}</span>{where_}{detail}</li>", inline(&s.step))
        })
        .collect();
    format!("<ol class=\"steps\">{lis}</ol>")
}

/// `sourceLine` (`minos-doc-html.ts:49`).
fn source_line(src: &Fonte) -> String {
    let parts: Vec<&str> = [non_empty(&src.repo), non_empty(&src.r#ref)]
        .into_iter()
        .flatten()
        .collect();
    let paths = if src.paths.is_empty() {
        String::new()
    } else {
        let codes: Vec<String> = src
            .paths
            .iter()
            .map(|p| format!("<code>{}</code>", esc(p)))
            .collect();
        format!(" · {}", codes.join(" "))
    };
    if parts.is_empty() && paths.is_empty() {
        return String::new();
    }
    format!(
        "<div class=\"meta mono\">{}{paths}</div>",
        esc(&parts.join(" @ "))
    )
}

/// `head` (`minos-doc-html.ts:55`).
fn head(
    title: &str,
    kicker: &str,
    generated_at: &str,
    scope: Escopo,
    lead: &str,
    extra: &str,
) -> String {
    format!(
        "<header><h1>{}</h1><div class=\"meta\">{} · {} · atualizado em {}</div>{extra}<p class=\"lead\">{}</p></header>",
        esc(title),
        esc(kicker),
        esc(scope_label(scope)),
        esc(&utc_text(generated_at)),
        inline(lead)
    )
}

/// `explanationDescription` (`minos-doc-html.ts:60`).
pub fn explanation_description(data: &Explicacao) -> String {
    format!(
        "Explicação de {}: {} no fluxo, {}, {}.",
        data.title,
        plural(data.flow.len(), "passo", "passos"),
        plural(data.sections.len(), "seção", "seções"),
        plural(data.files.len(), "arquivo", "arquivos")
    )
}

/// `renderExplanationBody` (`minos-doc-html.ts:65`).
pub fn render_explanation_body(data: &Explicacao) -> String {
    let mut parts = vec![head(
        &data.title,
        EXPLANATION_TITLE,
        &data.generated_at,
        data.scope,
        &data.summary,
        &source_line(&data.source),
    )];
    if let Some(q) = non_empty(&data.question) {
        parts.push(section("Pergunta", "", &prose(&[q.to_owned()])));
    }
    if !data.flow.is_empty() {
        parts.push(section(
            "Fluxo",
            &plural(data.flow.len(), "passo", "passos"),
            &step_list(&data.flow),
        ));
    }
    for s in &data.sections {
        let p = if s.paragraphs.is_empty() {
            String::new()
        } else {
            prose(&s.paragraphs)
        };
        parts.push(section(&s.title, "", &format!("{p}{}", snippets(&s.code))));
    }
    if !data.files.is_empty() {
        let rows: String = data
            .files
            .iter()
            .map(|f| {
                format!(
                    "<tr><td class=\"mono\">{}</td><td>{}</td></tr>",
                    esc(&f.path),
                    inline(&f.role)
                )
            })
            .collect();
        parts.push(section(
            "Arquivos",
            &plural(data.files.len(), "arquivo", "arquivos"),
            &format!("<div class=\"tablewrap\"><table><tr><th>Arquivo</th><th>Papel</th></tr>{rows}</table></div>"),
        ));
    }
    if !data.glossary.is_empty() {
        let lis: String = data
            .glossary
            .iter()
            .map(|g| {
                format!(
                    "<li class=\"row\"><strong>{}</strong><span class=\"grow\">{}</span></li>",
                    esc(&g.term),
                    inline(&g.meaning)
                )
            })
            .collect();
        parts.push(section("Glossário", "", &format!("<ul>{lis}</ul>")));
    }
    if !data.caveats.is_empty() {
        parts.push(section(
            "Ressalvas",
            "o que não foi verificado",
            &bullets(&data.caveats),
        ));
    }
    if !data.next_questions.is_empty() {
        parts.push(section(
            "Para investigar",
            "",
            &bullets(&data.next_questions),
        ));
    }
    parts.join("\n  ")
}

/// `openQuestions` (`minos-doc-html.ts:82`).
fn open_questions(data: &Proposta) -> usize {
    data.questions
        .iter()
        .filter(|q| q.status == StatusPergunta::Aberta)
        .count()
}

/// `proposalDescription` (`minos-doc-html.ts:84`).
pub fn proposal_description(data: &Proposta) -> String {
    format!(
        "{}: {} ({}), {}, {}.",
        proposal_title(data),
        data.title,
        status_label(data.status).0,
        plural(data.options.len(), "opção", "opções"),
        plural(open_questions(data), "pergunta aberta", "perguntas abertas")
    )
}

/// `optionCard` (`minos-doc-html.ts:89`).
fn option_card(o: &Opcao, chosen: bool) -> String {
    let badges: Vec<String> = [
        if chosen {
            badge("recomendada", "vivo")
        } else {
            String::new()
        },
        level_badge("esforço", o.effort),
        level_badge("risco", o.risk),
    ]
    .into_iter()
    .filter(|b| !b.is_empty())
    .collect();
    let badges = badges.join(" ");
    let mut cols = String::new();
    if !o.pros.is_empty() {
        cols.push_str(&format!("<div><h3>A favor</h3>{}</div>", bullets(&o.pros)));
    }
    if !o.cons.is_empty() {
        cols.push_str(&format!("<div><h3>Contra</h3>{}</div>", bullets(&o.cons)));
    }
    let touches = if o.touches.is_empty() {
        String::new()
    } else {
        let codes: Vec<String> = o
            .touches
            .iter()
            .map(|t| format!("<code>{}</code>", esc(t)))
            .collect();
        format!("<div class=\"meta\">toca: {}</div>", codes.join(" "))
    };
    format!(
        "<div class=\"option{}\"><div class=\"row\"><h3>{} · {}</h3>{}</div><p>{}</p>{}{touches}{}</div>",
        if chosen { " chosen" } else { "" },
        esc(&o.id),
        esc(&o.title),
        if badges.is_empty() {
            String::new()
        } else {
            format!("<span>{badges}</span>")
        },
        inline(&o.summary),
        if cols.is_empty() {
            String::new()
        } else {
            format!("<div class=\"cols\">{cols}</div>")
        },
        snippets(&o.sketch)
    )
}

/// `comparison` (`minos-doc-html.ts:97`).
fn comparison(data: &Proposta, chosen: Option<&str>) -> String {
    if data.options.len() < 2 {
        return String::new();
    }
    let cell = |l: Option<Nivel>| {
        l.map_or_else(
            || NO_VALUE.to_owned(),
            |l| {
                let (label, kind) = level_label(l);
                badge(label, kind)
            },
        )
    };
    let rows: String = data
        .options
        .iter()
        .map(|o| {
            let rec = if Some(o.id.as_str()) == chosen {
                format!(" {}", badge("recomendada", "vivo"))
            } else {
                String::new()
            };
            format!(
                "<tr><td>{}{rec}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&o.id),
                esc(&o.title),
                cell(o.effort),
                cell(o.risk)
            )
        })
        .collect();
    format!(
        "<div class=\"tablewrap\"><table><tr><th>Opção</th><th>Título</th><th>Esforço</th><th>Risco</th></tr>{rows}</table></div>"
    )
}

fn two_cols(a: (&str, String, bool), b: (&str, String, bool)) -> String {
    let col = |(h, inner, show): (&str, String, bool)| {
        if show {
            format!("<div><h3>{h}</h3>{inner}</div>")
        } else {
            String::new()
        }
    };
    format!("<div class=\"cols\">{}{}</div>", col(a), col(b))
}

/// `renderProposalBody` (`minos-doc-html.ts:105`).
pub fn render_proposal_body(data: &Proposta) -> String {
    let (status, status_kind) = status_label(data.status);
    let chosen = data
        .recommendation
        .as_ref()
        .and_then(|r| r.option.as_deref());
    let open: Vec<_> = data
        .questions
        .iter()
        .filter(|q| q.status == StatusPergunta::Aberta)
        .collect();
    let answered: Vec<_> = data
        .questions
        .iter()
        .filter(|q| q.status == StatusPergunta::Respondida)
        .collect();
    let extra = format!(
        "{}<div class=\"meta\">{}</div>",
        source_line(&data.source),
        badge(status, status_kind)
    );
    let mut parts = vec![head(
        &data.title,
        proposal_title(data),
        &data.generated_at,
        data.scope,
        &data.summary,
        &extra,
    )];
    if !open.is_empty() {
        let lis: String = open
            .iter()
            .map(|q| {
                format!(
                    "<li class=\"row\"><span class=\"mono muted\">{}</span><span class=\"grow\">{}</span></li>",
                    esc(&q.id),
                    inline(&q.question)
                )
            })
            .collect();
        parts.push(section(
            "Perguntas em aberto",
            &plural(open.len(), "aberta", "abertas"),
            &format!("<ul>{lis}</ul>"),
        ));
    }
    if !data.context.is_empty() {
        parts.push(section("Contexto", "", &prose(&data.context)));
    }
    if !data.problem.is_empty() {
        parts.push(section("Problema", "", &prose(&data.problem)));
    }
    if !data.goals.is_empty() || !data.non_goals.is_empty() {
        parts.push(section(
            "Escopo",
            "",
            &two_cols(
                ("Objetivos", bullets(&data.goals), !data.goals.is_empty()),
                (
                    "Fora do escopo",
                    bullets(&data.non_goals),
                    !data.non_goals.is_empty(),
                ),
            ),
        ));
    }
    if !data.current_flow.is_empty() || !data.proposed_flow.is_empty() {
        parts.push(section(
            "Fluxo",
            "",
            &two_cols(
                (
                    "Hoje",
                    step_list(&data.current_flow),
                    !data.current_flow.is_empty(),
                ),
                (
                    "Proposto",
                    step_list(&data.proposed_flow),
                    !data.proposed_flow.is_empty(),
                ),
            ),
        ));
    }
    if !data.options.is_empty() {
        let cards: String = data
            .options
            .iter()
            .map(|o| option_card(o, Some(o.id.as_str()) == chosen))
            .collect();
        parts.push(section(
            "Opções",
            &plural(data.options.len(), "opção", "opções"),
            &format!("{}{cards}", comparison(data, chosen)),
        ));
    }
    if let Some(rec) = &data.recommendation {
        let which = chosen
            .filter(|c| !c.is_empty())
            .map_or_else(String::new, |c| {
                let title = data
                    .options
                    .iter()
                    .find(|o| o.id == c)
                    .map_or("", |o| o.title.as_str());
                format!("<p><strong>{}</strong> · {}</p>", esc(c), esc(title))
            });
        let why = if rec.why.is_empty() {
            String::new()
        } else {
            bullets(&rec.why)
        };
        parts.push(section("Recomendação", "", &format!("{which}{why}")));
    }
    if !data.plan.is_empty() {
        parts.push(section(
            "Plano",
            &plural(data.plan.len(), "passo", "passos"),
            &step_list(&data.plan),
        ));
    }
    if !data.risks.is_empty() {
        let rows: String = data
            .risks
            .iter()
            .map(|r| {
                let m = non_empty(&r.mitigation).map_or_else(|| NO_VALUE.to_owned(), inline);
                format!("<tr><td>{}</td><td>{m}</td></tr>", inline(&r.risk))
            })
            .collect();
        parts.push(section(
            "Riscos",
            &plural(data.risks.len(), "risco", "riscos"),
            &format!("<div class=\"tablewrap\"><table><tr><th>Risco</th><th>Mitigação</th></tr>{rows}</table></div>"),
        ));
    }
    if !answered.is_empty() {
        let lis: String = answered
            .iter()
            .map(|q| {
                let answer =
                    non_empty(&q.answer).map_or_else(String::new, |a| format!("<p class=\"muted\">{}</p>", inline(a)));
                format!(
                    "<li><div class=\"row\"><span class=\"mono muted\">{}</span><span class=\"grow\">{}</span></div>{answer}</li>",
                    esc(&q.id),
                    inline(&q.question)
                )
            })
            .collect();
        parts.push(section(
            "Perguntas respondidas",
            "",
            &format!("<ul>{lis}</ul>"),
        ));
    }
    if !data.references.is_empty() {
        let lis: String = data
            .references
            .iter()
            .map(|r| {
                format!(
                    "<li class=\"row\"><span>{}</span><span class=\"grow mono muted\">{}</span></li>",
                    inline(&r.label),
                    esc(&r.r#ref)
                )
            })
            .collect();
        parts.push(section("Referências", "", &format!("<ul>{lis}</ul>")));
    }
    parts.join("\n  ")
}
