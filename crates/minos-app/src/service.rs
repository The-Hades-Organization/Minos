//! Leitura e gravação que o app faz sobre o `minos-core`; nada aqui roda na thread da interface. O app nunca publica:
//! ele lê o registro, grava o link que a sessão devolveu e abre a página.

use std::path::{Path, PathBuf};
use std::time::Duration;

use hades_core::Env;
use hades_core::clock::now_ms;
use hades_core::config::config_value;
use hades_core::fsx::read_text;
use hades_core::paths::{find_state_dir, skill_root};
use hades_core::proc::{RunOptions, run, which};
use minos_core::Escopo;
use minos_core::coleta::collect_from_env;
use minos_core::publicar::{Pedido, preparar, publish};
use minos_core::store::{ErroLink, Saidas, minos_dir, output_id};
use minos_core::{gravar_link, saidas};

use crate::nova::{self, Bloqueio, EstadoPrevia, Previa, Rascunho};
use crate::vm::{self, erro_de_saida, erro_de_url};

/// Pastas do app dentro de `$D/minos`: a prévia que abre no navegador e o HTML offline.
const PASTA_DA_PREVIA: &str = "previa";
const PASTA_OFFLINE: &str = "offline";

/// `OPEN_TIMEOUT` do CLI: o `xdg-open` não segura a interface.
const ABRIR_TIMEOUT: Duration = Duration::from_secs(10);

/// Tudo o que as telas leem, numa ida ao disco.
#[derive(Clone, Debug)]
pub struct Dados {
    pub saidas: Saidas,
    /// `HADES_MINOS_TEAM` efetivo (`commands/minos.ts:25`).
    pub time_ligado: bool,
    /// Pasta do registro (`$D/minos`), para o rodapé.
    pub pasta: PathBuf,
}

/// `stateDir` do CLI (`src/lib/kit.ts:27`), com o mesmo texto de recusa.
pub fn pasta_de_estado(env: &Env, cwd: &Path) -> Result<PathBuf, String> {
    find_state_dir(env, cwd).ok_or_else(|| {
        "sem instalação do Hades para guardar o estado: rode kit/install.sh (ou defina HADES_DATA)".to_owned()
    })
}

pub fn carregar(env: &Env, cwd: &Path) -> Result<Dados, String> {
    let state = pasta_de_estado(env, cwd)?;
    Ok(Dados {
        saidas: saidas(&state),
        time_ligado: vm::time_ligado(&config_value(env, cwd, "HADES_MINOS_TEAM", "0")),
        pasta: minos_dir(&state),
    })
}

/// `link` do CLI sem a interface: grava o link da saída e devolve o link normalizado.
pub fn gravar(env: &Env, cwd: &Path, id: &str, url: &str) -> Result<String, String> {
    let state = pasta_de_estado(env, cwd)?;
    gravar_link(&state, id, url).map_err(|e| match e {
        ErroLink::Url => erro_de_url(),
        ErroLink::SaidaDesconhecida => erro_de_saida(id),
        ErroLink::Io(e) => format!("não deu para gravar o registro: {e}"),
    })
}

/// `open` do CLI: entrega o alvo ao `xdg-open` (ou `open`); sem abridor no PATH, avisa.
pub fn abrir(env: &Env, alvo: &str) -> Result<(), String> {
    let path = env.get_raw("PATH").unwrap_or("");
    let Some(abridor) = ["xdg-open", "open"]
        .into_iter()
        .find(|c| which(c, path).is_some())
    else {
        return Err(format!("sem xdg-open nem open no PATH: abra {alvo}"));
    };
    let opcoes = RunOptions {
        timeout: Some(ABRIR_TIMEOUT),
        ..RunOptions::default()
    };
    run(abridor, &[alvo], &opcoes);
    Ok(())
}

/// Monta o pedido do `publish` como o CLI (`commands/minos.ts:72`) e entrega a `f`.
fn com_pedido<R>(
    env: &Env,
    cwd: &Path,
    r: &Rascunho,
    html_file: Option<String>,
    f: impl FnOnce(&Pedido) -> R,
) -> Result<R, String> {
    let fonte = r.fonte.as_ref().ok_or("escolha a fonte")?;
    let state = pasta_de_estado(env, cwd)?;
    let agora = now_ms(env).map_err(|e| format!("HADES_NOW inválido: {}", e.0))?;
    let coletor =
        |plan: &Path, scope: Escopo, now: i64| collect_from_env(plan, scope, now, env, cwd);
    let req = Pedido {
        model: r.modelo,
        plan: fonte.to_string_lossy().into_owned(),
        scope: r.escopo(),
        html_file,
        state: state.clone(),
        team_enabled: vm::time_ligado(&config_value(env, cwd, "HADES_MINOS_TEAM", "0")),
        credentials: read_text(&state.join("credenciais.md")),
        skill_roots: vec![skill_root(env)],
        cwd: cwd.to_path_buf(),
        now_ms: agora,
        collect: &coletor,
    };
    Ok(f(&req))
}

/// A prévia: o `publish` até a guarda, sem gravar nada. Recusa (inclusive da guarda) volta como estado da prévia.
pub fn previa(env: &Env, cwd: &Path, r: &Rascunho) -> Result<EstadoPrevia, String> {
    com_pedido(env, cwd, r, None, |req| match preparar(req, env) {
        Ok(p) => {
            let id = output_id(req.model, &req.plan, req.scope);
            EstadoPrevia::Pronta(nova::previa(id, &p.data, p.page))
        }
        Err(e) => EstadoPrevia::Recusada(nova::bloqueio(&e)),
    })
}

/// "Abrir no navegador": grava a página da prévia em `$D/minos/previa/<id>.html` e entrega ao abridor.
pub fn abrir_previa(env: &Env, cwd: &Path, p: &Previa) -> Result<PathBuf, String> {
    let dir = minos_dir(&pasta_de_estado(env, cwd)?).join(PASTA_DA_PREVIA);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let arquivo = dir.join(format!("{}.html", p.id));
    std::fs::write(&arquivo, &p.pagina).map_err(|e| format!("{}: {e}", arquivo.display()))?;
    abrir(env, &arquivo.to_string_lossy())?;
    Ok(arquivo)
}

/// Resultado de "Gerar": as linhas que o CLI imprime, ou a recusa.
pub type Gerado = Result<Vec<String>, Bloqueio>;

/// "Gerar" (`publish`) e "Gerar HTML" (`publish --html`). O HTML offline vai para `$D/minos/offline/<id>.html`.
pub fn gerar(env: &Env, cwd: &Path, r: &Rascunho, html: bool) -> Result<Gerado, String> {
    let html_file = if html {
        let fonte = r.fonte.as_ref().ok_or("escolha a fonte")?;
        let dir = minos_dir(&pasta_de_estado(env, cwd)?).join(PASTA_OFFLINE);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let id = output_id(r.modelo, &fonte.to_string_lossy(), r.escopo());
        Some(
            dir.join(format!("{id}.html"))
                .to_string_lossy()
                .into_owned(),
        )
    } else {
        None
    };
    com_pedido(env, cwd, r, html_file, |req| {
        publish(req, env)
            .map(|p| nova::linhas_do_gerado(&p))
            .map_err(|e| nova::bloqueio(&e))
    })
}

#[cfg(test)]
mod tests;
