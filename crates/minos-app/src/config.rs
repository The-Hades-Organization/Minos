//! Configurações do Minos: a chave `HADES_MINOS_TEAM` do catálogo `config.tsv`, lida e gravada só pela API do
//! `hades-core::config`, que grava em `$D/config.env` como o TS.

use std::path::Path;

use hades_core::Env;
use hades_core::config::{self, ConfigStore};
use hades_core::paths::{find_state_dir, skill_root};
use hades_ui::components::SettingOrigin;

use crate::vm;

pub const CHAVE: &str = "HADES_MINOS_TEAM";

/// A chave com o valor que vale agora.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Linha {
    pub rotulo: String,
    pub descricao: String,
    pub padrao: String,
    pub valor: String,
    pub origem: SettingOrigin,
}

impl Linha {
    pub fn ligado(&self) -> bool {
        vm::time_ligado(&self.valor)
    }

    pub fn somente_leitura(&self) -> bool {
        self.origem == SettingOrigin::Env
    }
}

/// Ponto único por onde a página lê e grava.
pub trait FonteDeConfig: Send + Sync {
    fn carregar(&self) -> Result<Linha, String>;
    fn gravar(&self, ligado: bool) -> Result<(), String>;
    /// `false` sem instalação do Hades: nada é gravado dentro da skill.
    fn gravavel(&self) -> bool;
}

/// Fonte padrão: o `$D` da instalação (`stateDir`) e o kit da skill.
pub struct ConfigLocal {
    env: Env,
    store: ConfigStore,
    instalado: bool,
}

impl ConfigLocal {
    pub fn new(env: &Env, cwd: &Path) -> Self {
        let kit = skill_root(env).join("kit");
        let estado = find_state_dir(env, cwd);
        Self {
            instalado: estado.is_some(),
            store: ConfigStore {
                dir: estado.unwrap_or_else(|| kit.clone()),
                kit,
            },
            env: env.clone(),
        }
    }
}

impl FonteDeConfig for ConfigLocal {
    fn carregar(&self) -> Result<Linha, String> {
        let efetiva = config::effective(&self.store)
            .map_err(|e| e.0)?
            .into_iter()
            .find(|e| e.item.key == CHAVE)
            .ok_or_else(|| format!("{CHAVE} não está no catálogo config.tsv da skill"))?;
        let no_arquivo = config::load_values(&self.store).has(CHAVE);
        let exportada = self.env.get_raw(CHAVE);
        let origem = match (exportada.is_some(), no_arquivo) {
            (true, _) => SettingOrigin::Env,
            (false, true) => SettingOrigin::File,
            (false, false) => SettingOrigin::Default,
        };
        Ok(Linha {
            valor: exportada.map_or(efetiva.value, str::to_owned),
            rotulo: efetiva.item.label,
            descricao: efetiva.item.desc,
            padrao: efetiva.item.default,
            origem,
        })
    }

    fn gravar(&self, ligado: bool) -> Result<(), String> {
        if !self.instalado {
            return Err("sem instalação do Hades: nada a gravar".to_owned());
        }
        let padrao = self.carregar()?.padrao;
        let valor = if ligado { "1" } else { "0" };
        let resultado = if valor == padrao {
            config::save(&self.store, &[], &[CHAVE])
        } else {
            config::save(&self.store, &[(CHAVE, valor)], &[])
        };
        resultado.map(|_| ()).map_err(|e| e.0)
    }

    fn gravavel(&self) -> bool {
        self.instalado
    }
}

/// Liga ou desliga a página do time e devolve o valor que passou a valer. Exportada no ambiente, a chave é só leitura.
pub fn alternar(fonte: &dyn FonteDeConfig, linha: &Linha, ligado: bool) -> Result<bool, String> {
    if linha.somente_leitura() {
        return Err(format!("{CHAVE} está exportada no ambiente: mude por lá"));
    }
    fonte.gravar(ligado)?;
    Ok(fonte.carregar()?.ligado())
}

#[cfg(test)]
mod tests;
