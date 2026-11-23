//! Tela Configurações: `HADES_MINOS_TEAM`, lida e gravada pela [`FonteDeConfig`] no executor de fundo.

use gpui::{Context, ParentElement};
use hades_ui::components::{
    EmptyState, Page, PageHeader, SettingOrigin, SettingRow, SettingsGroup, TermBlock,
};

use super::MinosView;
use crate::config::{self, CHAVE};

impl MinosView {
    pub(super) fn carregar_config(&mut self, cx: &mut Context<Self>) {
        self.t_config.invalidar();
        let Some(g) = self.t_config.iniciar() else {
            return;
        };
        let fonte = self.fonte_config.clone();
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_executor()
                .spawn(async move { fonte.carregar() })
                .await;
            let _ = this.update(cx, |v, cx| {
                if v.t_config.concluir(g) {
                    v.config = Some(r);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn alternar_time(&mut self, ligado: bool, cx: &mut Context<Self>) {
        let Some(Ok(linha)) = self.config.clone() else {
            return;
        };
        let Some(g) = self.t_config.iniciar() else {
            return;
        };
        let fonte = self.fonte_config.clone();
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_executor()
                .spawn(async move {
                    config::alternar(&*fonte, &linha, ligado).and_then(|_| fonte.carregar())
                })
                .await;
            let _ = this.update(cx, |v, cx| {
                if !v.t_config.concluir(g) {
                    return;
                }
                match r {
                    Ok(l) => {
                        v.config = Some(Ok(l));
                        v.erro_config = None;
                        v.forcar_recarga(cx);
                    }
                    Err(e) => v.erro_config = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn tela_config(&self, cx: &mut Context<Self>) -> Page {
        let cabecalho = PageHeader::new("Configurações")
            .subtitle("As chaves do Minos no config.env da instalação")
            .description(
                "Variável exportada no ambiente vence o arquivo e deixa a chave só leitura.",
            );
        let pagina = Page::new("pagina-config").header(cabecalho);
        let linha = match &self.config {
            None => return pagina.child(self.esqueleto("config")),
            Some(Err(e)) => {
                return pagina.child(
                    EmptyState::new("Não deu para ler a configuração")
                        .hint(e.clone())
                        .fill(),
                );
            }
            Some(Ok(l)) => l,
        };
        let gravavel = self.fonte_config.gravavel();
        let e = cx.entity();
        let linha_da_chave = SettingRow::switch("minos-team", linha.rotulo.clone(), linha.ligado())
            .key(CHAVE)
            .description(linha.descricao.clone())
            .default_value(linha.padrao.clone())
            .origin(linha.origem)
            .modified(linha.origem == SettingOrigin::File)
            .disabled(linha.somente_leitura() || !gravavel || self.t_config.carregando())
            .error_opt(self.erro_config.clone())
            .on_change(move |ligado, _, cx| e.update(cx, |v, cx| v.alternar_time(ligado, cx)));
        let pagina = pagina.child(SettingsGroup::new("Minos").row(linha_da_chave));
        if gravavel {
            pagina
        } else {
            pagina.child(
                TermBlock::new()
                    .warn("sem instalação do Hades: nada é gravado (rode kit/install.sh)"),
            )
        }
    }
}
