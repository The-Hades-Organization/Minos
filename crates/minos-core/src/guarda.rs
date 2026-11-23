//! `minos-guard.ts`: a guarda antes de toda publicação. Primeiro a varredura em memória ([`crate::varredura`]);
//! limpa, o momento `artefato` do Cerberus sobre a página numa pasta de preparo.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cerberus_core::{Cerberus, Entrada, ExecutorEmProcesso, Opcoes};
use hades_core::env::Env;
use serde_json::{Map, Value};

use crate::varredura::{ProjetoAtivo, Texto, guard_terms, scan_output};

const SEM_DETALHE: &str = "o Cerberus barrou sem detalhe";

/// `barredBy` (`minos-guard.ts:44`): as linhas `conferido` da evidência; sem elas, o detalhe.
pub fn barred_by(v: &Map<String, Value>) -> Vec<String> {
    if v.get("efeito").and_then(Value::as_str) != Some("barra") {
        return Vec::new();
    }
    let lines: Vec<String> = v
        .get("evidencia")
        .and_then(Value::as_array)
        .map(|ev| {
            ev.iter()
                .filter_map(|e| {
                    e.get("conferido")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default();
    if !lines.is_empty() {
        return lines;
    }
    vec![
        v.get("detalhe")
            .and_then(Value::as_str)
            .unwrap_or(SEM_DETALHE)
            .to_owned(),
    ]
}

/// `tmpdir()` do Node: `TMPDIR` sem a barra final; sem ele, `/tmp`.
fn tmp_root(env: &Env) -> PathBuf {
    match env
        .get("TMPDIR")
        .map(|t| t.trim_end_matches('/'))
        .filter(|t| !t.is_empty())
    {
        Some(t) => PathBuf::from(t),
        None => PathBuf::from("/tmp"),
    }
}

/// `mkdtempSync(join(tmpdir(), "minos-"))`.
fn stage_dir(env: &Env) -> io::Result<PathBuf> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let root = tmp_root(env);
    loop {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let seed = u64::from(nanos)
            ^ (u64::from(std::process::id()) << 32)
            ^ SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = root.join(format!("minos-{:06x}", seed & 0xff_ffff));
        match std::fs::create_dir(&dir) {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            r => return r.map(|()| dir),
        }
    }
}

/// `cerberusReasons` (`minos-guard.ts:52`): o Cerberus sobre a página; a pasta de preparo some em qualquer caso.
/// O TS deixa a exceção do Cerberus subir; aqui ela volta como `Err`.
pub fn cerberus_reasons(page: &str, cwd: &Path, env: &Env) -> Result<Vec<String>, String> {
    let dir = stage_dir(env).map_err(|e| format!("pasta de preparo: {e}"))?;
    let result = evaluate(page, &dir, cwd, env);
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn evaluate(page: &str, dir: &Path, cwd: &Path, env: &Env) -> Result<Vec<String>, String> {
    std::fs::write(dir.join("index.html"), page).map_err(|e| format!("pasta de preparo: {e}"))?;
    let cerberus = Cerberus::abrir(Opcoes::novo(env.clone(), cwd.to_path_buf()))
        .com_executor(Arc::new(ExecutorEmProcesso::novo()));
    let mut entrada = Entrada::default();
    entrada
        .ctx
        .insert("arquivo".into(), dir.to_string_lossy().into_owned());
    let result = cerberus
        .avaliar("artefato", entrada)
        .map_err(|e| e.to_string())?;
    if !result.barrou() {
        return Ok(Vec::new());
    }
    Ok(result.vereditos.iter().flat_map(barred_by).collect())
}

/// `GuardRequest` (`minos-guard.ts:64`).
#[derive(Clone, Debug, Default)]
pub struct Pedido {
    pub page: String,
    pub data: String,
    pub credentials: Option<String>,
    pub skill_roots: Vec<PathBuf>,
    pub cwd: PathBuf,
    /// O projeto ativo, dispensado da varredura de nome (só página pessoal de explicação/proposta).
    pub except_project: Option<ProjetoAtivo>,
}

/// `guardPublication` (`minos-guard.ts:75`): memória e depois Cerberus. Lista vazia = liberado.
pub fn guard_publication(req: &Pedido, env: &Env) -> Result<Vec<String>, String> {
    let texts = [
        Texto {
            label: "página".into(),
            text: req.page.clone(),
        },
        Texto {
            label: "dados".into(),
            text: req.data.clone(),
        },
    ];
    let terms = guard_terms(env, &req.skill_roots, req.except_project.as_ref());
    let memory = scan_output(&texts, req.credentials.as_deref(), &terms)?;
    if !memory.is_empty() {
        return Ok(memory);
    }
    cerberus_reasons(&req.page, &req.cwd, env)
}

#[cfg(test)]
#[path = "guarda_tests.rs"]
mod tests;
