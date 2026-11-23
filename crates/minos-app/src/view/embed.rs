//! O Minos dentro do painel do Hades: telas do trilho, comandos da paleta e a view sem trilho nem janela.

use std::path::PathBuf;

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use hades_core::Env;
use hades_ui::icons::IconName;
use hades_ui::{Comando, Embutida, FerramentaEmbutida, FerramentaInfo, TelaInfo};

use super::MinosView;
use crate::vm::Tela;

const fn tela_info(t: Tela) -> TelaInfo {
    TelaInfo {
        id: t.id(),
        rotulo: t.rotulo(),
        icone: t.icone(),
    }
}

/// As telas do trilho próprio, na mesma ordem.
const TELAS: &[TelaInfo] = &[
    tela_info(Tela::Publicacoes),
    tela_info(Tela::NovaPagina),
    tela_info(Tela::Time),
    tela_info(Tela::Configuracoes),
];

/// As telas quando as Configurações vêm do painel.
const TELAS_INTEGRADO: &[TelaInfo] = &[
    tela_info(Tela::Publicacoes),
    tela_info(Tela::NovaPagina),
    tela_info(Tela::Time),
];

const RECARREGAR: &str = "publicacoes.recarregar";
const ABRIR_PREVIA: &str = "nova.abrir-previa";
const GERAR: &str = "nova.gerar";
const GERAR_HTML: &str = "nova.gerar-html";

/// Os botões das telas Publicações, Página do time e Nova página.
const COMANDOS: &[Comando] = &[
    Comando {
        id: RECARREGAR,
        grupo: "Publicações",
        rotulo: "Recarregar as publicações",
        descricao: Some("Relê o registro de saídas e o estado da página do time"),
        icone: IconName::RefreshCw,
        atalho: None,
    },
    Comando {
        id: ABRIR_PREVIA,
        grupo: "Nova página",
        rotulo: "Abrir a prévia no navegador",
        descricao: Some("Abre a prévia pronta do rascunho"),
        icone: IconName::Eye,
        atalho: None,
    },
    Comando {
        id: GERAR,
        grupo: "Nova página",
        rotulo: "Gerar a página",
        descricao: Some("Gera a saída do rascunho com a prévia pronta"),
        icone: IconName::Play,
        atalho: None,
    },
    Comando {
        id: GERAR_HTML,
        grupo: "Nova página",
        rotulo: "Gerar o HTML",
        descricao: Some("Gera o HTML do rascunho com a prévia pronta"),
        icone: IconName::Download,
        atalho: None,
    },
];

/// Telas que a view mostra com as opções `opts`.
pub fn telas(opts: Embutida) -> &'static [TelaInfo] {
    if opts.integrado {
        TELAS_INTEGRADO
    } else {
        TELAS
    }
}

impl MinosView {
    /// Tela de id `id` entre as visíveis; id desconhecido, ou Configurações com o painel integrado, dá a primeira.
    fn tela_visivel(&self, id: &str) -> Tela {
        telas(Embutida {
            integrado: self.integrado,
        })
        .iter()
        .find(|t| t.id == id)
        .and_then(|t| Tela::de_nome(t.id))
        .unwrap_or(Tela::Publicacoes)
    }
}

impl FerramentaEmbutida for MinosView {
    /// Ambiente do processo e pasta do projeto.
    type Ambiente = (Env, PathBuf);

    fn info() -> FerramentaInfo {
        FerramentaInfo {
            id: "minos",
            titulo: crate::NOME,
            icone: IconName::Scale,
            telas: TELAS,
            comandos: COMANDOS,
        }
    }

    fn embutida(
        (env, cwd): Self::Ambiente,
        foco: Option<&str>,
        opts: Embutida,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut v = Self::new(env, cwd, cx.focus_handle(), cx);
        v.integrado = opts.integrado;
        if let Some(id) = foco {
            let t = v.tela_visivel(id);
            if t != v.tela {
                v.mudar_tela(t, cx);
            }
        }
        v
    }

    fn render_tela(
        &mut self,
        tela: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.garantir_campo(window, cx);
        let t = self.tela_visivel(tela);
        if t != self.tela {
            self.mudar_tela(t, cx);
        }
        let corpo = self.corpo(t, cx);
        div()
            .size_full()
            .flex()
            .min_h_0()
            .child(corpo)
            .into_any_element()
    }

    fn executar(&mut self, comando: &str, _window: &mut Window, cx: &mut Context<Self>) {
        let pronta = self.previa_pronta().is_some();
        match comando {
            RECARREGAR => self.forcar_recarga(cx),
            ABRIR_PREVIA if pronta => self.abrir_previa(cx),
            GERAR if pronta => self.gerar_pagina(false, cx),
            GERAR_HTML if pronta => self.gerar_pagina(true, cx),
            _ => return,
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_sem_ids_repetidos() {
        assert!(MinosView::info().ids_repetidos().is_empty());
    }

    #[test]
    fn toda_tela_do_trilho_tem_tela_info() {
        let info = MinosView::info();
        for t in Tela::TODAS {
            let tela = info
                .tela(t.id())
                .unwrap_or_else(|| panic!("sem TelaInfo: {}", t.rotulo()));
            assert_eq!((tela.rotulo, tela.icone), (t.rotulo(), t.icone()));
        }
        assert_eq!(info.telas.len(), Tela::TODAS.len());
    }

    #[test]
    fn ids_das_telas_valem_na_flag_tela() {
        for t in MinosView::info().telas {
            assert_eq!(Tela::de_nome(t.id).map(Tela::id), Some(t.id));
        }
    }

    #[test]
    fn integrado_tira_configuracoes() {
        let ids = |opts| telas(opts).iter().map(|t| t.id).collect::<Vec<_>>();
        assert!(ids(Embutida::default()).contains(&Tela::Configuracoes.id()));
        let integrado = ids(Embutida { integrado: true });
        assert!(!integrado.contains(&Tela::Configuracoes.id()));
        assert_eq!(integrado.len(), Tela::TODAS.len() - 1);
    }
}
