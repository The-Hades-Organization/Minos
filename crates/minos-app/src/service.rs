//! Leitura e gravação que o app faz sobre o `minos-core`; nada aqui roda na thread da interface. O app nunca publica:
//! ele lê o registro, grava o link que a sessão devolveu e abre a página.

use std::path::{Path, PathBuf};
use std::time::Duration;

use hades_core::Env;
use hades_core::config::config_value;
use hades_core::paths::find_state_dir;
use hades_core::proc::{RunOptions, run, which};
use minos_core::store::{ErroLink, Saidas, minos_dir};
use minos_core::{gravar_link, saidas};

use crate::vm::{self, erro_de_saida, erro_de_url};

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

#[cfg(test)]
mod tests;
