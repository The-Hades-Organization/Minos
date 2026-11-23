//! Registro das saídas (`src/lib/minos-store.ts`): `$D/minos/outputs.json` e a pasta de cada saída (`$D/minos/<saída>/`).
//! O link estável de cada saída mora aqui: a próxima publicação atualiza no mesmo lugar.
//!
//! O TS grava o registro sem lock (LS:62); aqui toda gravação passa pelo flock de `$D/minos/.lock` (plano F4 §2.1). O
//! formato é o mesmo (`JSON.stringify(_, null, 2)` + `\n`), então cada lado lê o que o outro gravou.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use hades_core::atomic::write_json_atomic;
use hades_core::fsx::read_text;
use hades_core::lock::with_flock;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::check::Escopo;
use crate::modelo::Modelo;

/// `ARTIFACT_HOST` (`minos-store.ts:10`).
pub const ARTIFACT_HOST: &str = "claude.ai";
const OUTPUT_ID_MAX: usize = 81;
const BASE_MAX: usize = 60;

/// `OutputRecord` (`minos-store.ts:12`); serializa com as chaves na ordem do TS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Saida {
    pub model: Modelo,
    pub scope: Escopo,
    pub plan: String,
    pub url: Option<String>,
    pub file: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

/// Saídas na ordem em que o `Object.entries` do TS as devolve.
pub type Saidas = Vec<(String, Saida)>;

/// Recusa do [`gravar_link`], na ordem de conferência do `link` (`commands/minos.ts:97`).
#[derive(Debug)]
pub enum ErroLink {
    /// Fora de `https://claude.ai/[code/]artifact/<id>`.
    Url,
    /// Id inválido ou sem registro.
    SaidaDesconhecida,
    Io(io::Error),
}

/// `minosDir` (`minos-store.ts:21`).
pub fn minos_dir(state: &Path) -> PathBuf {
    state.join("minos")
}

fn registry_file(state: &Path) -> PathBuf {
    minos_dir(state).join("outputs.json")
}

fn lock_file(state: &Path) -> PathBuf {
    minos_dir(state).join(".lock")
}

/// `isOutputId` (`minos-store.ts:24`): `^[a-z0-9][a-z0-9-]{0,80}$`.
pub fn is_output_id(id: &str) -> bool {
    let b = id.as_bytes();
    let ok = |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    !b.is_empty()
        && b.len() <= OUTPUT_ID_MAX
        && ok(&b[0])
        && b[1..].iter().all(|c| ok(c) || *c == b'-')
}

/// `outputId` (`minos-store.ts:29`): `<modelo>-<fonte>` (e `-team` na página do time). A mesma fonte e escopo caem
/// sempre na mesma saída.
pub fn output_id(model: Modelo, plan_path: &str, scope: Escopo) -> String {
    let name = plan_path.rsplit('/').next().unwrap_or("");
    let lower_name = name.to_ascii_lowercase();
    let stem = [".md", ".json"]
        .iter()
        .find(|ext| lower_name.ends_with(*ext))
        .map_or(name, |ext| &name[..name.len() - ext.len()]);
    let mut base = String::new();
    for c in stem.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            base.push(c);
        } else if !base.ends_with('-') {
            base.push('-');
        }
    }
    let base: String = base.trim_matches('-').chars().take(BASE_MAX).collect();
    let base = if base.is_empty() { "plano" } else { &base };
    let team = if scope == Escopo::Time { "-team" } else { "" };
    format!("{}-{base}{team}", model.id())
}

fn is_artifact_path(path: &str) -> bool {
    let rest = path
        .strip_prefix("/code/")
        .or_else(|| path.strip_prefix('/'));
    let Some(id) = rest.and_then(|r| r.strip_prefix("artifact/")) else {
        return false;
    };
    let id = id.strip_suffix('/').unwrap_or(id);
    !id.is_empty()
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

/// `parseArtifactUrl` (`minos-store.ts:35`): só `https://claude.ai/…/artifact/<id>`; host vindo de dado nunca é usado
/// sem lista permitida. O `url` segue o WHATWG URL, como o `new URL` do TS (porta padrão some, consulta e fragmento
/// não contam).
pub fn parse_artifact_url(raw: &str) -> Option<String> {
    let url = url::Url::parse(raw).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some(ARTIFACT_HOST)
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some_and(|p| !p.is_empty())
        || !is_artifact_path(url.path())
    {
        return None;
    }
    let path = url.path();
    Some(format!(
        "https://{ARTIFACT_HOST}{}",
        path.strip_suffix('/').unwrap_or(path)
    ))
}

fn texto(r: &Map<String, Value>, key: &str) -> Option<String> {
    r.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// `record` (`minos-store.ts:46`): entrada inválida vira `None` em silêncio; `url` inválida vira `null`.
fn record(value: &Value) -> Option<Saida> {
    let r = value.as_object()?;
    let model = r
        .get("model")
        .and_then(Value::as_str)
        .and_then(Modelo::from_id)?;
    let scope = r
        .get("scope")
        .and_then(Value::as_str)
        .and_then(Escopo::parse)?;
    let file = texto(r, "file")?;
    let plan = texto(r, "plan")?;
    let updated_at = texto(r, "updatedAt")?;
    let url = r
        .get("url")
        .and_then(Value::as_str)
        .and_then(parse_artifact_url);
    Some(Saida {
        model,
        scope,
        plan,
        url,
        file,
        updated_at,
    })
}

/// Índice de array canônico do JS (`0` a `2^32 - 2`, sem zero à esquerda): essas chaves vêm antes das outras, em ordem
/// numérica, em qualquer objeto.
fn js_index(key: &str) -> Option<u32> {
    if key.len() > 1 && key.starts_with('0') {
        return None;
    }
    key.parse::<u32>().ok().filter(|n| *n != u32::MAX)
}

/// Ordem das chaves de um objeto JS: índices primeiro (numérica), o resto na ordem de inserção.
fn js_key_order<T>(entries: Vec<(String, T)>) -> Vec<(String, T)> {
    let (mut idx, rest): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .partition(|(k, _)| js_index(k).is_some());
    idx.sort_by_key(|(k, _)| js_index(k));
    idx.extend(rest);
    idx
}

/// `readOutputs` (`minos-store.ts:56`). Como o `Object.entries` do TS, um `outputs` em array vale pelos índices.
pub fn read_outputs(state: &Path) -> Saidas {
    let parsed = read_text(&registry_file(state))
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .filter(Value::is_object);
    let entries: Vec<(String, &Value)> = match parsed.as_ref().and_then(|d| d.get("outputs")) {
        Some(Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Some(Value::Array(a)) => a
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        _ => return Vec::new(),
    };
    let saidas = entries
        .into_iter()
        .filter(|(id, _)| is_output_id(id))
        .filter_map(|(id, v)| record(v).map(|r| (id, r)))
        .collect();
    js_key_order(saidas)
}

/// `saidas(state)` do plano (§3.1).
pub fn saidas(state: &Path) -> Saidas {
    read_outputs(state)
}

fn write_registry(state: &Path, saidas: &Saidas) -> io::Result<()> {
    let outputs: Map<String, Value> = saidas
        .iter()
        .map(|(id, s)| (id.clone(), serde_json::to_value(s).unwrap_or(Value::Null)))
        .collect();
    let mut doc = Map::new();
    doc.insert("outputs".to_owned(), Value::Object(outputs));
    write_json_atomic(&registry_file(state), &Value::Object(doc))
}

fn upsert(saidas: &mut Saidas, id: &str, rec: Saida) {
    match saidas.iter_mut().find(|(k, _)| k == id) {
        Some(slot) => slot.1 = rec,
        None => saidas.push((id.to_owned(), rec)),
    }
}

fn locked<T>(state: &Path, f: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    fs::create_dir_all(minos_dir(state))?;
    with_flock(&lock_file(state), f)?
}

/// `saveOutput` (`minos-store.ts:62`), sob o flock de `$D/minos/.lock`: lê, mescla e grava sem perder a saída de quem
/// gravou ao mesmo tempo.
pub fn save_output(state: &Path, id: &str, rec: Saida) -> io::Result<()> {
    locked(state, || {
        let mut all = read_outputs(state);
        upsert(&mut all, id, rec);
        write_registry(state, &js_key_order(all))
    })
}

/// `gravar_link(state, id, url)` do plano (§3.1): o `link` (`commands/minos.ts:97`) sem a interface. Devolve o link
/// normalizado que ficou gravado.
pub fn gravar_link(state: &Path, id: &str, url: &str) -> Result<String, ErroLink> {
    let url = parse_artifact_url(url).ok_or(ErroLink::Url)?;
    if !is_output_id(id) {
        return Err(ErroLink::SaidaDesconhecida);
    }
    locked(state, || {
        let mut all = read_outputs(state);
        let Some((_, current)) = all.iter_mut().find(|(k, _)| k == id) else {
            return Ok(Err(ErroLink::SaidaDesconhecida));
        };
        current.url = Some(url.clone());
        write_registry(state, &all).map(|()| Ok(url.clone()))
    })
    .map_err(ErroLink::Io)?
}

/// `writeOutputFiles` (`minos-store.ts:68`): grava a página e os dados da saída; devolve o caminho do `index.html`.
pub fn write_output_files(state: &Path, id: &str, page: &str, data: &str) -> io::Result<PathBuf> {
    let dir = minos_dir(state).join(id);
    fs::create_dir_all(&dir)?;
    let file = dir.join("index.html");
    fs::write(&file, page)?;
    fs::write(dir.join("data.json"), data)?;
    Ok(file)
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
