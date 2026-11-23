//! Modelos de documento `explicacao` e `proposta` (`src/lib/minos-doc.ts`).
//!
//! A sessão escreve o JSON; aqui ele é conferido na fronteira. `title` e `summary` são obrigatórios; lista ausente vira
//! lista vazia e campo opcional ausente vira `None`, nunca valor inventado. A ordem de avaliação segue a do TS.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::{Value, json};

use crate::check::{
    ContractError, Escopo, LIST_MAX, Obj, Resultado, TEXT_MAX, enum_texto, list, obj, one_of, text,
};
use crate::modelo::{Modelo, conferir_modelo, escopo};

/// `minos-doc.ts:6`.
pub const EXPLICACAO_SCHEMA: u64 = 1;
/// `minos-doc.ts:7`.
pub const PROPOSTA_SCHEMA: u64 = 1;
/// `minos-doc.ts:8`.
pub const PARAGRAPH_MAX: usize = 4000;
/// `minos-doc.ts:9`.
pub const SNIPPET_MAX: usize = 12000;
/// `minos-doc.ts:10`.
const SNIPPETS_MAX: usize = 40;

enum_texto!(
    /// `minos-doc.ts:12`.
    TipoProposta, PROPOSAL_KINDS {
        Implementacao => "implementacao", Correcao => "correcao", Fluxo => "fluxo", Refatoracao => "refatoracao",
        Discussao => "discussao",
    }
);

enum_texto!(
    /// `minos-doc.ts:14`.
    StatusProposta, PROPOSAL_STATUSES {
        Rascunho => "rascunho", EmDiscussao => "em-discussao", Decidida => "decidida", Descartada => "descartada",
    }
);

enum_texto!(
    /// `minos-doc.ts:16`.
    Nivel, LEVELS { Baixo => "baixo", Medio => "medio", Alto => "alto" }
);

enum_texto!(
    /// `minos-doc.ts:18`.
    StatusPergunta, QUESTION_STATUSES { Aberta => "aberta", Respondida => "respondida" }
);

/// `minos-doc.ts:21`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fonte {
    pub repo: Option<String>,
    pub r#ref: Option<String>,
    pub paths: Vec<String>,
}

/// `minos-doc.ts:22`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Trecho {
    pub file: String,
    pub lines: Option<String>,
    pub lang: Option<String>,
    pub snippet: String,
    pub note: Option<String>,
}

/// `minos-doc.ts:23`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Passo {
    pub step: String,
    pub r#where: Option<String>,
    pub detail: Option<String>,
}

/// `minos-doc.ts:24`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Secao {
    pub title: String,
    pub paragraphs: Vec<String>,
    pub code: Vec<Trecho>,
}

/// `minos-doc.ts:25`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PapelArquivo {
    pub path: String,
    pub role: String,
}

/// `minos-doc.ts:26`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Termo {
    pub term: String,
    pub meaning: String,
}

/// Dados do modelo `explicacao` (`minos-doc.ts:28`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Explicacao {
    pub model: Modelo,
    pub schema: u64,
    pub scope: Escopo,
    pub generated_at: String,
    pub title: String,
    pub question: Option<String>,
    pub summary: String,
    pub source: Fonte,
    pub flow: Vec<Passo>,
    pub sections: Vec<Secao>,
    pub files: Vec<PapelArquivo>,
    pub glossary: Vec<Termo>,
    pub caveats: Vec<String>,
    pub next_questions: Vec<String>,
}

/// `minos-doc.ts:45`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Opcao {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
    pub effort: Option<Nivel>,
    pub risk: Option<Nivel>,
    pub touches: Vec<String>,
    pub sketch: Vec<Trecho>,
}

/// `minos-doc.ts:49`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Risco {
    pub risk: String,
    pub mitigation: Option<String>,
}

/// `minos-doc.ts:50`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pergunta {
    pub id: String,
    pub question: String,
    pub status: StatusPergunta,
    pub answer: Option<String>,
}

/// `minos-doc.ts:51`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Referencia {
    pub label: String,
    pub r#ref: String,
}

/// `minos-doc.ts:70`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Recomendacao {
    pub option: Option<String>,
    pub why: Vec<String>,
}

/// Dados do modelo `proposta` (`minos-doc.ts:53`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposta {
    pub model: Modelo,
    pub schema: u64,
    pub scope: Escopo,
    pub generated_at: String,
    pub kind: TipoProposta,
    pub status: StatusProposta,
    pub title: String,
    pub summary: String,
    pub source: Fonte,
    pub context: Vec<String>,
    pub problem: Vec<String>,
    pub goals: Vec<String>,
    pub non_goals: Vec<String>,
    pub current_flow: Vec<Passo>,
    pub proposed_flow: Vec<Passo>,
    pub options: Vec<Opcao>,
    pub recommendation: Option<Recomendacao>,
    pub plan: Vec<Passo>,
    pub risks: Vec<Risco>,
    pub questions: Vec<Pergunta>,
    pub references: Vec<Referencia>,
}

fn ausente(value: Option<&Value>) -> bool {
    matches!(value, None | Some(Value::Null))
}

/// `optText` (`minos-doc.ts:77`).
fn opt_text(value: Option<&Value>, path: &str, max: usize) -> Resultado<Option<String>> {
    if ausente(value) {
        Ok(None)
    } else {
        text(value, path, max).map(Some)
    }
}

/// `optList` (`minos-doc.ts:78`).
fn opt_list<'a>(value: Option<&'a Value>, path: &str, max: usize) -> Resultado<&'a [Value]> {
    if ausente(value) {
        Ok(&[])
    } else {
        list(value, path, max)
    }
}

/// `optList(...).map(...)` com o caminho `path[i]` de cada item.
fn cada<T>(
    value: Option<&Value>,
    path: &str,
    max: usize,
    f: impl Fn(&Obj, &str) -> Resultado<T>,
) -> Resultado<Vec<T>> {
    opt_list(value, path, max)?
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let at = format!("{path}[{i}]");
            f(obj(Some(v), &at)?, &at)
        })
        .collect()
}

/// `texts` (`minos-doc.ts:79`).
fn texts(value: Option<&Value>, path: &str, max: usize) -> Resultado<Vec<String>> {
    opt_list(value, path, LIST_MAX)?
        .iter()
        .enumerate()
        .map(|(i, t)| text(Some(t), &format!("{path}[{i}]"), max))
        .collect()
}

/// `paragraphs` (`minos-doc.ts:80`).
fn paragraphs(value: Option<&Value>, path: &str) -> Resultado<Vec<String>> {
    texts(value, path, PARAGRAPH_MAX)
}

/// `optLevel` (`minos-doc.ts:81`).
fn opt_level(value: Option<&Value>, path: &str) -> Resultado<Option<Nivel>> {
    if ausente(value) {
        return Ok(None);
    }
    Ok(Some(Nivel::parse(one_of(value, LEVELS, path)?).unwrap()))
}

fn campo(x: &Obj, at: &str, k: &str, max: usize) -> Resultado<String> {
    text(x.get(k), &format!("{at}.{k}"), max)
}

fn opt_campo(x: &Obj, at: &str, k: &str, max: usize) -> Resultado<Option<String>> {
    opt_text(x.get(k), &format!("{at}.{k}"), max)
}

/// `minos-doc.ts:89`.
fn source(value: Option<&Value>) -> Resultado<Fonte> {
    if ausente(value) {
        return Ok(Fonte {
            repo: None,
            r#ref: None,
            paths: vec![],
        });
    }
    let o = obj(value, "source")?;
    Ok(Fonte {
        repo: opt_text(o.get("repo"), "source.repo", TEXT_MAX)?,
        r#ref: opt_text(o.get("ref"), "source.ref", TEXT_MAX)?,
        paths: texts(o.get("paths"), "source.paths", TEXT_MAX)?,
    })
}

/// `minos-doc.ts:95`.
fn code(value: Option<&Value>, path: &str) -> Resultado<Vec<Trecho>> {
    cada(value, path, SNIPPETS_MAX, |x, at| {
        Ok(Trecho {
            file: campo(x, at, "file", TEXT_MAX)?,
            lines: opt_campo(x, at, "lines", TEXT_MAX)?,
            lang: opt_campo(x, at, "lang", TEXT_MAX)?,
            snippet: campo(x, at, "snippet", SNIPPET_MAX)?,
            note: opt_campo(x, at, "note", PARAGRAPH_MAX)?,
        })
    })
}

/// `minos-doc.ts:105`.
fn steps(value: Option<&Value>, path: &str) -> Resultado<Vec<Passo>> {
    cada(value, path, LIST_MAX, |x, at| {
        Ok(Passo {
            step: campo(x, at, "step", TEXT_MAX)?,
            r#where: opt_campo(x, at, "where", TEXT_MAX)?,
            detail: opt_campo(x, at, "detail", PARAGRAPH_MAX)?,
        })
    })
}

/// `pairs` (`minos-doc.ts:112`): `k` com o teto padrão, `v` com `v_max`.
fn pairs(
    value: Option<&Value>,
    path: &str,
    k: &str,
    v: &str,
    v_max: usize,
) -> Resultado<Vec<(String, String)>> {
    cada(value, path, LIST_MAX, |x, at| {
        Ok((campo(x, at, k, TEXT_MAX)?, campo(x, at, v, v_max)?))
    })
}

fn cabecalho(input: &Value, model: Modelo, schema: u64) -> Resultado<(&Obj, Escopo, String)> {
    let o = obj(Some(input), "$")?;
    conferir_modelo(o, model.id(), schema)?;
    let (scope, generated_at) = escopo(o)?;
    Ok((o, scope, generated_at))
}

/// `validateExplanation` (`minos-doc.ts:119`).
pub fn validar_explicacao(input: &Value) -> Resultado<Explicacao> {
    let (o, scope, generated_at) = cabecalho(input, Modelo::Explicacao, EXPLICACAO_SCHEMA)?;
    Ok(Explicacao {
        model: Modelo::Explicacao,
        schema: EXPLICACAO_SCHEMA,
        scope,
        generated_at,
        title: text(o.get("title"), "title", TEXT_MAX)?,
        question: opt_text(o.get("question"), "question", PARAGRAPH_MAX)?,
        summary: text(o.get("summary"), "summary", PARAGRAPH_MAX)?,
        source: source(o.get("source"))?,
        flow: steps(o.get("flow"), "flow")?,
        sections: cada(o.get("sections"), "sections", LIST_MAX, |x, at| {
            Ok(Secao {
                title: campo(x, at, "title", TEXT_MAX)?,
                paragraphs: paragraphs(x.get("paragraphs"), &format!("{at}.paragraphs"))?,
                code: code(x.get("code"), &format!("{at}.code"))?,
            })
        })?,
        files: pairs(o.get("files"), "files", "path", "role", TEXT_MAX)?
            .into_iter()
            .map(|(path, role)| PapelArquivo { path, role })
            .collect(),
        glossary: pairs(
            o.get("glossary"),
            "glossary",
            "term",
            "meaning",
            PARAGRAPH_MAX,
        )?
        .into_iter()
        .map(|(term, meaning)| Termo { term, meaning })
        .collect(),
        caveats: paragraphs(o.get("caveats"), "caveats")?,
        next_questions: paragraphs(o.get("nextQuestions"), "nextQuestions")?,
    })
}

/// `minos-doc.ts:135`.
fn options(value: Option<&Value>) -> Resultado<Vec<Opcao>> {
    let mut seen = HashSet::new();
    opt_list(value, "options", LIST_MAX)?
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let at = &format!("options[{i}]");
            let x = obj(Some(p), at)?;
            let id = campo(x, at, "id", TEXT_MAX)?;
            if !seen.insert(id.clone()) {
                return Err(ContractError::new(format!("{at}.id"), "id repetido"));
            }
            Ok(Opcao {
                id,
                title: campo(x, at, "title", TEXT_MAX)?,
                summary: campo(x, at, "summary", PARAGRAPH_MAX)?,
                pros: paragraphs(x.get("pros"), &format!("{at}.pros"))?,
                cons: paragraphs(x.get("cons"), &format!("{at}.cons"))?,
                effort: opt_level(x.get("effort"), &format!("{at}.effort"))?,
                risk: opt_level(x.get("risk"), &format!("{at}.risk"))?,
                touches: texts(x.get("touches"), &format!("{at}.touches"), TEXT_MAX)?,
                sketch: code(x.get("sketch"), &format!("{at}.sketch"))?,
            })
        })
        .collect()
}

/// `minos-doc.ts:152`.
fn recommendation(value: Option<&Value>, opts: &[Opcao]) -> Resultado<Option<Recomendacao>> {
    if ausente(value) {
        return Ok(None);
    }
    let o = obj(value, "recommendation")?;
    let option = opt_text(o.get("option"), "recommendation.option", TEXT_MAX)?;
    if let Some(id) = &option
        && !opts.iter().any(|x| &x.id == id)
    {
        return Err(ContractError::new(
            "recommendation.option",
            "não é o id de nenhuma opção",
        ));
    }
    Ok(Some(Recomendacao {
        option,
        why: paragraphs(o.get("why"), "recommendation.why")?,
    }))
}

/// `o[k] ?? padrao` seguido de `oneOf`.
fn enum_ou<'a>(
    o: &'a Obj,
    k: &str,
    padrao: &'a Value,
    allowed: &[&str],
    path: &str,
) -> Resultado<&'a str> {
    let v = o.get(k).filter(|v| !v.is_null()).unwrap_or(padrao);
    one_of(Some(v), allowed, path)
}

/// `validateProposal` (`minos-doc.ts:160`).
pub fn validar_proposta(input: &Value) -> Resultado<Proposta> {
    let (o, scope, generated_at) = cabecalho(input, Modelo::Proposta, PROPOSTA_SCHEMA)?;
    let opts = options(o.get("options"))?;
    let kind = TipoProposta::parse(one_of(o.get("kind"), PROPOSAL_KINDS, "kind")?).unwrap();
    let rascunho = json!("rascunho");
    let status = StatusProposta::parse(enum_ou(
        o,
        "status",
        &rascunho,
        PROPOSAL_STATUSES,
        "status",
    )?)
    .unwrap();
    let title = text(o.get("title"), "title", TEXT_MAX)?;
    let summary = text(o.get("summary"), "summary", PARAGRAPH_MAX)?;
    let source = source(o.get("source"))?;
    let context = paragraphs(o.get("context"), "context")?;
    let problem = paragraphs(o.get("problem"), "problem")?;
    let goals = paragraphs(o.get("goals"), "goals")?;
    let non_goals = paragraphs(o.get("nonGoals"), "nonGoals")?;
    let current_flow = steps(o.get("currentFlow"), "currentFlow")?;
    let proposed_flow = steps(o.get("proposedFlow"), "proposedFlow")?;
    let recommendation = recommendation(o.get("recommendation"), &opts)?;
    let plan = steps(o.get("plan"), "plan")?;
    let risks = cada(o.get("risks"), "risks", LIST_MAX, |x, at| {
        Ok(Risco {
            risk: campo(x, at, "risk", PARAGRAPH_MAX)?,
            mitigation: opt_campo(x, at, "mitigation", PARAGRAPH_MAX)?,
        })
    })?;
    let aberta = json!("aberta");
    let questions = cada(o.get("questions"), "questions", LIST_MAX, |x, at| {
        let id = campo(x, at, "id", TEXT_MAX)?;
        let question = campo(x, at, "question", PARAGRAPH_MAX)?;
        let status = enum_ou(
            x,
            "status",
            &aberta,
            QUESTION_STATUSES,
            &format!("{at}.status"),
        )?;
        Ok(Pergunta {
            id,
            question,
            status: StatusPergunta::parse(status).unwrap(),
            answer: opt_campo(x, at, "answer", PARAGRAPH_MAX)?,
        })
    })?;
    let references = pairs(o.get("references"), "references", "label", "ref", TEXT_MAX)?
        .into_iter()
        .map(|(label, r#ref)| Referencia { label, r#ref })
        .collect();
    Ok(Proposta {
        model: Modelo::Proposta,
        schema: PROPOSTA_SCHEMA,
        scope,
        generated_at,
        kind,
        status,
        title,
        summary,
        source,
        context,
        problem,
        goals,
        non_goals,
        current_flow,
        proposed_flow,
        options: opts,
        recommendation,
        plan,
        risks,
        questions,
        references,
    })
}

/// `EXPLICACAO_EXAMPLE` (`minos-doc.ts:192`): esqueleto de `schema explicacao`; `scope` e `generatedAt` a CLI preenche.
pub fn exemplo_explicacao() -> Value {
    json!({
        "model": "explicacao", "schema": EXPLICACAO_SCHEMA,
        "title": "Como o pedido vira registro", "question": "O que acontece entre a rota POST /pedidos e a gravação no banco?",
        "summary": "A rota valida o corpo e entrega ao handler, que grava o pedido e o evento na mesma transação. Corpo com campo desconhecido volta 400 antes de chegar ao handler.",
        "source": { "repo": "dono/repo", "ref": "main@abc1234", "paths": ["src/pedidos/"] },
        "flow": [
            { "step": "A rota valida o corpo", "where": "src/pedidos/rota.ts:42", "detail": "Campo desconhecido volta 400 aqui." },
            { "step": "O handler grava pedido e evento numa transação", "where": "src/pedidos/criar.handler.ts:18", "detail": null },
        ],
        "sections": [{
            "title": "Validação",
            "paragraphs": ["`valida` recusa o corpo inteiro no primeiro campo inválido; a resposta lista só esse campo."],
            "code": [{ "file": "src/pedidos/rota.ts", "lines": "40-48", "lang": "ts", "snippet": "export function valida(corpo: unknown): Pedido {\n  return esquema.parse(corpo);\n}", "note": "A recusa acontece no parse." }],
        }],
        "files": [{ "path": "src/pedidos/rota.ts", "role": "entrada HTTP e validação" }, { "path": "src/pedidos/criar.handler.ts", "role": "transação de gravação" }],
        "glossary": [{ "term": "handler", "meaning": "a classe que executa um caso de uso" }],
        "caveats": ["Não conferi o comportamento com o banco fora do ar: o código não trata esse caso e não há teste dele."],
        "nextQuestions": ["Quem consome o evento gravado na transação?"],
    })
}

/// `PROPOSTA_EXAMPLE` (`minos-doc.ts:212`).
pub fn exemplo_proposta() -> Value {
    json!({
        "model": "proposta", "schema": PROPOSTA_SCHEMA, "kind": "correcao", "status": "em-discussao",
        "title": "Reprocessar pedidos presos",
        "summary": "Proponho tentar de novo com espera crescente (opção A): resolve o caso medido sem operação manual e muda só o consumidor.",
        "source": { "repo": "dono/repo", "ref": "main@abc1234", "paths": ["src/pedidos/"] },
        "context": ["Em 3 de novembro, 41 pedidos ficaram presos depois de uma queda do banco de 2 minutos (log do consumidor)."],
        "problem": ["O consumidor descarta a mensagem na primeira falha (`src/pedidos/consumidor.ts:88`), então uma queda curta perde o pedido."],
        "goals": ["Pedido preso por falha transitória volta a andar sozinho."], "nonGoals": ["Mudar o formato do evento."],
        "currentFlow": [{ "step": "O consumidor falha e descarta a mensagem", "where": "src/pedidos/consumidor.ts:88", "detail": null }],
        "proposedFlow": [{ "step": "O consumidor falha e agenda nova tentativa", "where": "src/pedidos/consumidor.ts", "detail": "Espera de 1, 4 e 16 s; depois da terceira, vai para a fila de mortos." }],
        "options": [
            {
                "id": "A", "title": "Nova tentativa com espera crescente", "summary": "Três tentativas no próprio consumidor antes da fila de mortos.",
                "pros": ["Cobre a queda de 2 minutos medida."], "cons": ["Segura a partição durante as esperas."],
                "effort": "medio", "risk": "baixo", "touches": ["src/pedidos/consumidor.ts"], "sketch": [],
            },
            {
                "id": "B", "title": "Fila de mortos e reprocesso manual", "summary": "Toda falha vai direto para a fila de mortos; alguém reprocessa.",
                "pros": ["Não segura a partição."], "cons": ["Depende de alguém olhar a fila."],
                "effort": "baixo", "risk": "medio", "touches": ["src/pedidos/consumidor.ts"], "sketch": [],
            },
        ],
        "recommendation": { "option": "A", "why": ["A queda medida dura menos que a soma das esperas (21 s), então a maioria se resolve sem gente."] },
        "plan": [{ "step": "Spec da nova tentativa", "where": null, "detail": "Antes do código." }, { "step": "Teste com o banco derrubado no meio do consumo", "where": null, "detail": null }],
        "risks": [{ "risk": "Esperas somadas atrasam a partição inteira", "mitigation": "Teto de 21 s por mensagem; acima disso, fila de mortos." }],
        "questions": [{ "id": "Q1", "question": "Quedas acima de 21 s acontecem? Não achei métrica disso.", "status": "aberta", "answer": null }],
        "references": [{ "label": "Log do consumidor de 3/11", "ref": "observabilidade, consulta salva pedidos-presos" }],
    })
}

#[cfg(test)]
#[path = "documento_tests.rs"]
mod tests;
