//! `minos-publish.ts`: o caminho de `hades minos publish`. A ordem é a garantia: dados validados, página montada, guarda
//! (nada gravado se barrar) e só então os arquivos e o registro. Nada aqui fala com o claude.ai.

use std::path::{Path, PathBuf};

use hades_core::env::Env;
use hades_core::fsx::{read_text, resolve};
use hades_core::jsonfmt::json_indent;
use hades_core::registry::iso_from_epoch;
use regex::Regex;
use serde_json::Value;

use crate::check::{ContractError, Escopo, js_len};
use crate::coleta::ErroColeta;
use crate::guarda::{Pedido as PedidoGuarda, guard_publication};
use crate::modelo::{Modelo, Origem, Orquestracao, Validado};
use crate::pagina::{description_of, load_template, render_page};
use crate::store::{Saida, output_id, read_outputs, save_output, write_output_files};
use crate::varredura::active_project_terms;

/// `DATA_FILE_MAX` (`minos-publish.ts:38`), em unidades UTF-16 como o `length` do JS.
pub const DATA_FILE_MAX: usize = 2 * 1024 * 1024;

const TEAM_OFF: [&str; 2] = [
    "a página do time está desligada (HADES_MINOS_TEAM=0): ela expõe o estado do trabalho a terceiros.",
    "Ligue no onboarding (passo \"Página do time\") ou em Configurações → Minos.",
];

/// `Refusal["kind"]` (`minos-publish.ts:14`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoRecusa {
    TeamOff,
    Contract,
    Guard,
    Input,
}

impl TipoRecusa {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TeamOff => "team-off",
            Self::Contract => "contract",
            Self::Guard => "guard",
            Self::Input => "input",
        }
    }
}

/// `Refusal` (`minos-publish.ts:14`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recusa {
    pub kind: TipoRecusa,
    pub reasons: Vec<String>,
}

fn refuse(kind: TipoRecusa, reasons: impl IntoIterator<Item = String>) -> Recusa {
    Recusa {
        kind,
        reasons: reasons.into_iter().collect(),
    }
}

/// `Published` (`minos-publish.ts:15`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Publicado {
    Offline {
        file: PathBuf,
    },
    Artifact {
        id: String,
        file: PathBuf,
        url: Option<String>,
        description: String,
        scope: Escopo,
    },
}

/// O coletor da orquestração (`collectOrchestration`), injetado: plano, escopo e o instante em ms.
pub type Coletor<'a> = &'a dyn Fn(&Path, Escopo, i64) -> Result<Orquestracao, ErroColeta>;

/// `PublishRequest` (`minos-publish.ts:19`). `plan` é o caminho como veio da invocação.
pub struct Pedido<'a> {
    pub model: Modelo,
    pub plan: String,
    pub scope: Escopo,
    pub html_file: Option<String>,
    pub state: PathBuf,
    pub team_enabled: bool,
    pub credentials: Option<String>,
    pub skill_roots: Vec<PathBuf>,
    pub cwd: PathBuf,
    pub now_ms: i64,
    pub collect: Coletor<'a>,
}

enum ErroDados {
    Contrato(ContractError),
    Entrada(String),
}

impl From<ContractError> for ErroDados {
    fn from(e: ContractError) -> Self {
        Self::Contrato(e)
    }
}

fn now_iso(now_ms: i64) -> String {
    iso_from_epoch(now_ms.div_euclid(1000))
}

fn at(cwd: &Path, path: &str) -> PathBuf {
    resolve(&cwd.join(path))
}

/// `loadDocument` (`minos-publish.ts:41`): `scope` vem da invocação e `generatedAt`, se faltar, é agora.
fn load_document(
    model: Modelo,
    path: &str,
    cwd: &Path,
    scope: Escopo,
    now_ms: i64,
) -> Result<Validado, ErroDados> {
    let raw = read_text(&at(cwd, path))
        .ok_or_else(|| ErroDados::Entrada(format!("dados não encontrados: {path}")))?;
    if js_len(&raw) > DATA_FILE_MAX {
        return Err(ErroDados::Entrada(format!(
            "dados acima de {DATA_FILE_MAX} bytes: {path}"
        )));
    }
    let parsed: Value = serde_json::from_str(&raw).map_err(|_| {
        ErroDados::Entrada(format!(
            "dados não são JSON válido: {path} (o exemplo sai em hades minos schema {})",
            model.id()
        ))
    })?;
    let Value::Object(mut input) = parsed else {
        return Err(ContractError::new("$", "esperava um objeto").into());
    };
    if !input.contains_key("generatedAt") {
        input.insert("generatedAt".into(), Value::String(now_iso(now_ms)));
    }
    input.insert("scope".into(), Value::String(scope.as_str().into()));
    Ok(model.validar(&Value::Object(input))?)
}

/// `writableHtml` (`minos-publish.ts:55`).
fn writable_html(file: &Path) -> Option<String> {
    let html = Regex::new(r"(?i)\.html?$").expect("padrão fixo");
    if !html.is_match(&file.to_string_lossy()) {
        return Some("--html: o arquivo precisa terminar em .html".into());
    }
    let dir = file.parent().unwrap_or(Path::new("/"));
    if !dir.is_dir() {
        return Some(format!("--html: a pasta {} não existe", dir.display()));
    }
    if file.is_dir() {
        return Some(format!("--html: {} é uma pasta", file.display()));
    }
    None
}

fn load_data(req: &Pedido) -> Result<Validado, ErroDados> {
    match req.model.origem() {
        Origem::Plano => (req.collect)(&at(&req.cwd, &req.plan), req.scope, req.now_ms)
            .map(Validado::Orquestracao)
            .map_err(|e| match e {
                ErroColeta::Contrato(c) => ErroDados::Contrato(c),
                other => ErroDados::Entrada(other.to_string()),
            }),
        Origem::Json => load_document(req.model, &req.plan, &req.cwd, req.scope, req.now_ms),
    }
}

fn io_input(what: &str, e: impl std::fmt::Display) -> Recusa {
    refuse(TipoRecusa::Input, [format!("{what}: {e}")])
}

/// O que o `publish` monta antes de gravar: os dados validados e a página, já liberados pela guarda.
#[derive(Clone, Debug, PartialEq)]
pub struct Preparado {
    pub data: Validado,
    pub page: String,
    pub data_json: String,
}

/// `publish` (`minos-publish.ts:63-89`) até a guarda, sem gravar nada: a prévia do app usa só esta parte.
pub fn preparar(req: &Pedido, env: &Env) -> Result<Preparado, Recusa> {
    if req.scope == Escopo::Time && !req.team_enabled {
        return Err(refuse(TipoRecusa::TeamOff, TEAM_OFF.map(str::to_owned)));
    }
    if let Some(problem) = html_target(req).as_deref().and_then(writable_html) {
        return Err(refuse(TipoRecusa::Input, [problem]));
    }
    let data = load_data(req).map_err(|e| match e {
        ErroDados::Contrato(c) => refuse(
            TipoRecusa::Contract,
            [format!("os dados não cumprem o contrato: {c}")],
        ),
        ErroDados::Entrada(m) => refuse(TipoRecusa::Input, [m]),
    })?;
    let template = load_template(env).map_err(|e| io_input("modelo da página", e))?;
    let page = render_page(&template, &data);
    let data_json = json_indent(&data.to_value(), 2);
    let except_project = (req.model.origem() == Origem::Json && req.scope == Escopo::Pessoal)
        .then(|| active_project_terms(env, &req.cwd))
        .flatten();
    let guard = PedidoGuarda {
        page: page.clone(),
        data: data_json.clone(),
        credentials: req.credentials.clone(),
        skill_roots: req.skill_roots.clone(),
        cwd: req.cwd.clone(),
        except_project,
    };
    let reasons = guard_publication(&guard, env).map_err(|e| refuse(TipoRecusa::Input, [e]))?;
    if !reasons.is_empty() {
        return Err(refuse(TipoRecusa::Guard, reasons));
    }
    Ok(Preparado {
        data,
        page,
        data_json,
    })
}

fn html_target(req: &Pedido) -> Option<PathBuf> {
    req.html_file.as_deref().map(|h| at(&req.cwd, h))
}

/// `publish` (`minos-publish.ts:63`). O TS deixa subir as falhas de E/S e do Cerberus; aqui viram recusa `input`.
pub fn publish(req: &Pedido, env: &Env) -> Result<Publicado, Recusa> {
    let Preparado {
        data,
        page,
        data_json,
    } = preparar(req, env)?;
    let html_file = html_target(req);

    if let Some(file) = html_file {
        std::fs::write(&file, &page).map_err(|e| io_input(&file.display().to_string(), e))?;
        return Ok(Publicado::Offline { file });
    }
    let id = output_id(req.model, &req.plan, req.scope);
    let file = write_output_files(&req.state, &id, &page, &format!("{data_json}\n"))
        .map_err(|e| io_input("saída", e))?;
    let url = read_outputs(&req.state)
        .into_iter()
        .find(|(k, _)| k == &id)
        .and_then(|(_, s)| s.url);
    let record = Saida {
        model: req.model,
        scope: req.scope,
        plan: at(&req.cwd, &req.plan).to_string_lossy().into_owned(),
        url: url.clone(),
        file: file.to_string_lossy().into_owned(),
        updated_at: now_iso(req.now_ms),
    };
    save_output(&req.state, &id, record).map_err(|e| io_input("registro", e))?;
    Ok(Publicado::Artifact {
        id,
        file,
        url,
        description: description_of(&data),
        scope: req.scope,
    })
}

#[cfg(test)]
#[path = "publicar_tests.rs"]
mod tests;
