//! Estado da tela Nova página, sem gpui: a escolha do modelo, da fonte e do escopo, a prévia montada dos dados
//! validados e os textos que o `publish` do CLI imprime (`commands/minos.ts:72`).

use std::path::PathBuf;

use hades_core::jsonfmt::json_indent;
use minos_core::Escopo;
use minos_core::modelo::{Modelo, Origem, Validado, exemplo, modelos};
use minos_core::pagina::{description_of, title_of};
use minos_core::publicar::{Publicado, Recusa, TipoRecusa};
use serde_json::{Map, Value};

/// Linhas de uma seção da prévia antes do "e mais N".
pub const LINHAS_POR_SECAO: usize = 12;

const CABECALHO: [&str; 6] = [
    "model",
    "schema",
    "scope",
    "generatedAt",
    "title",
    "summary",
];
const CAMPOS_DE_NOME: [&str; 10] = [
    "title", "step", "name", "question", "risk", "term", "path", "label", "file", "id",
];
const CAMPOS_DE_DETALHE: [&str; 10] = [
    "summary",
    "detail",
    "meaning",
    "role",
    "status",
    "mitigation",
    "answer",
    "ref",
    "where",
    "note",
];

/// O que a tela pede ao `publish`: modelo, arquivo de origem e escopo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rascunho {
    pub modelo: Modelo,
    pub fonte: Option<PathBuf>,
    pub time: bool,
}

impl Default for Rascunho {
    fn default() -> Self {
        Self {
            modelo: Modelo::Explicacao,
            fonte: None,
            time: false,
        }
    }
}

impl Rascunho {
    pub fn escopo(&self) -> Escopo {
        if self.time {
            Escopo::Time
        } else {
            Escopo::Pessoal
        }
    }

    /// O comando do CLI que faz o mesmo que "Gerar" (ou "Gerar HTML", com `html`).
    pub fn comando(&self, html: Option<&str>) -> Option<String> {
        let fonte = self.fonte.as_ref()?;
        let mut partes = vec![
            "hades minos publish".to_owned(),
            self.modelo.id().to_owned(),
            fonte.display().to_string(),
        ];
        if self.time {
            partes.push("--team".to_owned());
        }
        if let Some(h) = html {
            partes.push(format!("--html {h}"));
        }
        Some(partes.join(" "))
    }
}

pub fn rotulos_dos_modelos() -> Vec<&'static str> {
    modelos().iter().map(|m| m.title()).collect()
}

pub fn modelo_de_indice(i: usize) -> Modelo {
    modelos().get(i).copied().unwrap_or(Modelo::Explicacao)
}

pub fn indice_do_modelo(m: Modelo) -> usize {
    modelos().iter().position(|x| *x == m).unwrap_or(0)
}

/// O que o seletor de fonte pede para o modelo.
pub fn dica_da_fonte(m: Modelo) -> &'static str {
    match m.origem() {
        Origem::Plano => "O plano da orquestração (.md)",
        Origem::Json => "Os dados do modelo (.json), no formato do exemplo",
    }
}

/// O exemplo que `hades minos schema <modelo>` imprime; o modelo que sai do plano não tem.
pub fn exemplo_do_modelo(m: Modelo) -> Option<String> {
    exemplo(m).map(|v| json_indent(&v, 2))
}

/// Uma seção da prévia: título e uma linha por item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecaoPrevia {
    pub titulo: String,
    pub linhas: Vec<String>,
}

/// A página montada a partir dos dados validados, liberada pela guarda.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Previa {
    pub id: String,
    pub titulo: String,
    pub descricao: String,
    pub secoes: Vec<SecaoPrevia>,
    pub pagina: String,
}

/// Recusa do `publish` com o texto do CLI. O motivo da guarda já vem como `arquivo:linha  tipo`, sem o trecho.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bloqueio {
    pub cabeca: String,
    pub motivos: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EstadoPrevia {
    Pronta(Previa),
    Recusada(Bloqueio),
}

/// A cabeça que o `publish` do CLI imprime antes dos motivos.
pub fn bloqueio(r: &Recusa) -> Bloqueio {
    let cabeca = if r.kind == TipoRecusa::Guard {
        "o Cerberus barrou a publicação (nada foi gerado). Limpe e rode de novo:"
    } else {
        "minos: recusado:"
    };
    Bloqueio {
        cabeca: cabeca.to_owned(),
        motivos: r.reasons.clone(),
    }
}

pub fn previa(id: String, data: &Validado, pagina: String) -> Previa {
    Previa {
        id,
        titulo: titulo_de(data),
        descricao: description_of(data),
        secoes: secoes(&data.to_value()),
        pagina,
    }
}

/// O título dos dados (`title`) ou, sem ele, o do modelo.
fn titulo_de(data: &Validado) -> String {
    match data.to_value().get("title").and_then(Value::as_str) {
        Some(t) if !t.trim().is_empty() => t.to_owned(),
        _ => title_of(data).to_owned(),
    }
}

/// As seções da prévia, na ordem das chaves dos dados: o resumo, os campos simples numa seção "Dados" e uma seção
/// por lista ou objeto. Lista vazia não vira seção.
pub fn secoes(dados: &Value) -> Vec<SecaoPrevia> {
    let Some(obj) = dados.as_object() else {
        return Vec::new();
    };
    let mut saida = Vec::new();
    if let Some(r) = obj.get("summary").and_then(Value::as_str) {
        saida.push(secao("Resumo", vec![r.to_owned()]));
    }
    let simples: Vec<String> = obj
        .iter()
        .filter(|(k, v)| !CABECALHO.contains(&k.as_str()) && escalar(v).is_some())
        .map(|(k, v)| format!("{}: {}", rotulo_da_chave(k), escalar(v).unwrap_or_default()))
        .collect();
    if !simples.is_empty() {
        saida.push(secao("Dados", simples));
    }
    for (k, v) in obj {
        if CABECALHO.contains(&k.as_str()) {
            continue;
        }
        let linhas = match v {
            Value::Array(itens) => itens.iter().map(resumo_do_item).collect(),
            Value::Object(o) => campos(o),
            _ => continue,
        };
        if !linhas.is_empty() {
            saida.push(secao(rotulo_da_chave(k), linhas));
        }
    }
    saida
}

fn secao(titulo: &str, mut linhas: Vec<String>) -> SecaoPrevia {
    if linhas.len() > LINHAS_POR_SECAO {
        let resto = linhas.len() - LINHAS_POR_SECAO;
        linhas.truncate(LINHAS_POR_SECAO);
        linhas.push(format!("e mais {resto}"));
    }
    SecaoPrevia {
        titulo: titulo.to_owned(),
        linhas,
    }
}

fn escalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "sim" } else { "não" }.to_owned()),
        _ => None,
    }
}

/// O primeiro campo escalar não vazio da lista, com a chave.
fn primeiro<'a>(o: &Map<String, Value>, chaves: &[&'a str]) -> Option<(&'a str, String)> {
    chaves.iter().find_map(|k| {
        let v = o.get(*k).and_then(escalar)?;
        (!v.trim().is_empty()).then_some((*k, v))
    })
}

/// Um item de lista numa linha: o nome e, se houver, o detalhe.
fn resumo_do_item(v: &Value) -> String {
    let Value::Object(o) = v else {
        return escalar(v).unwrap_or_else(|| v.to_string());
    };
    let Some((chave, nome)) = primeiro(o, &CAMPOS_DE_NOME) else {
        return v.to_string();
    };
    let outros: Vec<&str> = CAMPOS_DE_DETALHE
        .into_iter()
        .filter(|k| *k != chave)
        .collect();
    match primeiro(o, &outros).map(|(_, d)| d) {
        Some(d) => format!("{nome} — {d}"),
        None => nome,
    }
}

/// Os campos de um objeto: escalar como `chave: valor`, lista como contagem.
fn campos(o: &Map<String, Value>) -> Vec<String> {
    o.iter()
        .filter_map(|(k, v)| {
            let valor = match v {
                Value::Array(a) if a.iter().all(|x| x.is_string()) && !a.is_empty() => a
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
                Value::Array(a) if !a.is_empty() => crate::vm::itens(a.len()),
                _ => escalar(v)?,
            };
            Some(format!("{}: {valor}", rotulo_da_chave(k)))
        })
        .collect()
}

/// Rótulo pt-BR das chaves conhecidas dos três modelos; o resto aparece como veio.
pub fn rotulo_da_chave(k: &str) -> &str {
    match k {
        "question" => "Pergunta",
        "source" => "Fonte",
        "flow" => "Fluxo",
        "sections" => "Seções",
        "files" => "Arquivos",
        "glossary" => "Glossário",
        "caveats" => "Ressalvas",
        "nextQuestions" => "Próximas perguntas",
        "kind" => "Tipo",
        "status" => "Situação",
        "problem" => "Problema",
        "goals" => "Objetivos",
        "nonGoals" => "Fora do escopo",
        "currentFlow" => "Fluxo atual",
        "proposedFlow" => "Fluxo proposto",
        "context" => "Contexto",
        "options" => "Opções",
        "recommendation" => "Recomendação",
        "plan" => "Plano",
        "steps" => "Passos",
        "risks" => "Riscos",
        "questions" => "Perguntas",
        "references" => "Referências",
        "agents" => "Agentes",
        "prs" => "PRs",
        "escalations" => "Escalações",
        "consumption" => "Consumo",
        "repo" => "Repositório",
        "ref" => "Referência",
        "paths" => "Caminhos",
        "why" => "Por quê",
        "option" => "Opção",
        outra => outra,
    }
}

/// O que o `publish` do CLI imprime quando grava (`commands/minos.ts:83-94`): o que a sessão precisa para publicar.
pub fn linhas_do_gerado(p: &Publicado) -> Vec<String> {
    match p {
        Publicado::Offline { file } => vec![
            format!("arquivo: {}", file.display()),
            "Página autocontida (sem script nem recurso externo); nada saiu da máquina.".to_owned(),
        ],
        Publicado::Artifact {
            id,
            file,
            url,
            description,
            ..
        } => vec![
            format!("publique: {}", file.display()),
            format!(
                "link registrado: {}",
                url.as_deref().unwrap_or("nenhum (primeira publicação)")
            ),
            format!("descrição: {description}"),
            format!("depois: hades minos link {id} <url devolvida pela ferramenta Artifact>"),
        ],
    }
}

#[cfg(test)]
mod tests;
