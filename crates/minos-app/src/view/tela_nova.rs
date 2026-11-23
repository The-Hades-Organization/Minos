//! Tela Nova página: modelo, fonte e escopo; a prévia é recalculada no fundo a cada mudança e "Gerar" roda o
//! `publish` do CLI.

use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use hades_ui::components::{
    Button, Card, CopyButton, EmptyState, Page, PageHeader, PathField, Segmented, Switch, TermBlock,
};
use hades_ui::icons::IconName;
use hades_ui::tokens::space;

use super::MinosView;
use crate::nova::{self, Bloqueio, EstadoPrevia, LINHAS_POR_SECAO, Previa, Rascunho, SecaoPrevia};
use crate::service;

impl MinosView {
    fn mudar_rascunho(&mut self, f: impl FnOnce(&mut Rascunho), cx: &mut Context<Self>) {
        f(&mut self.rascunho);
        self.gerado = None;
        self.t_gerar.invalidar();
        self.recalcular_previa(cx);
    }

    pub(super) fn recalcular_previa(&mut self, cx: &mut Context<Self>) {
        self.t_previa.invalidar();
        self.previa = None;
        cx.notify();
        if self.rascunho.fonte.is_none() {
            return;
        }
        let Some(g) = self.t_previa.iniciar() else {
            return;
        };
        let (env, cwd, r) = (self.env.clone(), self.cwd.clone(), self.rascunho.clone());
        cx.spawn(async move |this, cx| {
            let estado = cx
                .background_executor()
                .spawn(async move { service::previa(&env, &cwd, &r) })
                .await;
            let _ = this.update(cx, |v, cx| {
                if v.t_previa.concluir(g) {
                    v.previa = Some(estado);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn previa_pronta(&self) -> Option<&Previa> {
        match &self.previa {
            Some(Ok(EstadoPrevia::Pronta(p))) => Some(p),
            _ => None,
        }
    }

    fn gerar_pagina(&mut self, html: bool, cx: &mut Context<Self>) {
        let Some(g) = self.t_gerar.iniciar() else {
            return;
        };
        self.gerado = None;
        let (env, cwd, r) = (self.env.clone(), self.cwd.clone(), self.rascunho.clone());
        cx.spawn(async move |this, cx| {
            let gerado = cx
                .background_executor()
                .spawn(async move { service::gerar(&env, &cwd, &r, html) })
                .await;
            let _ = this.update(cx, |v, cx| {
                if !v.t_gerar.concluir(g) {
                    return;
                }
                let ok = matches!(gerado, Ok(Ok(_)));
                v.gerado = Some(gerado);
                if ok {
                    v.forcar_recarga(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn abrir_previa(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.previa_pronta().cloned() else {
            return;
        };
        let (env, cwd) = (self.env.clone(), self.cwd.clone());
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_executor()
                .spawn(async move { service::abrir_previa(&env, &cwd, &p) })
                .await;
            if let Err(e) = r {
                let _ = this.update(cx, |v, cx| {
                    v.gerado = Some(Err(e));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(super) fn tela_nova(&self, cx: &mut Context<Self>) -> Page {
        let cabecalho = PageHeader::new("Nova página")
            .subtitle("Modelo, fonte e escopo; a prévia mostra o que a página vai ter")
            .description("Gerar prepara a página como o hades minos publish; a sessão publica com a ferramenta Artifact.");
        Page::new("pagina-nova")
            .header(cabecalho)
            .child(self.escolha(cx))
            .child(self.previa_view())
            .child(self.acoes(cx))
            .child(self.resultado())
    }

    fn escolha(&self, cx: &mut Context<Self>) -> Card {
        let r = &self.rascunho;
        let time_ligado = self.dados.as_ref().is_some_and(|d| d.time_ligado);
        let (e_modelo, e_fonte, e_time) = (cx.entity(), cx.entity(), cx.entity());
        let modelos = Segmented::new("modelos", nova::rotulos_dos_modelos())
            .selected(nova::indice_do_modelo(r.modelo))
            .on_select(move |i, _, cx| {
                e_modelo.update(cx, |v, cx| {
                    v.mudar_rascunho(|r| r.modelo = nova::modelo_de_indice(i), cx);
                });
            });
        let fonte = PathField::new("fonte")
            .value(
                r.fonte
                    .as_ref()
                    .map(|f| f.display().to_string())
                    .unwrap_or_default(),
            )
            .placeholder(nova::dica_da_fonte(r.modelo))
            .prompt("Escolher")
            .on_pick(move |caminho, cx| {
                e_fonte.update(cx, |v, cx| {
                    v.mudar_rascunho(|r| r.fonte = Some(caminho), cx)
                });
            });
        let time = Switch::new("escopo-time", r.time && time_ligado)
            .label("Publicar na página do time")
            .disabled(!time_ligado)
            .on_change(move |ligado, _, cx| {
                e_time.update(cx, |v, cx| v.mudar_rascunho(|r| r.time = ligado, cx));
            });
        let mut cartao = Card::new()
            .title("Origem")
            .child(modelos)
            .child(fonte)
            .child(time);
        if !time_ligado {
            cartao = cartao
                .note("HADES_MINOS_TEAM desligado: só o escopo pessoal. Ligue em Configurações.");
        }
        if let Some(exemplo) = nova::exemplo_do_modelo(r.modelo) {
            cartao = cartao.action(
                CopyButton::new("copiar-exemplo", exemplo)
                    .label("Copiar exemplo")
                    .copied_label("Copiado"),
            );
        }
        cartao
    }

    fn previa_view(&self) -> gpui::AnyElement {
        if self.rascunho.fonte.is_none() {
            return EmptyState::new("Escolha a fonte para ver a prévia")
                .hint(nova::dica_da_fonte(self.rascunho.modelo))
                .into_any_element();
        }
        match &self.previa {
            None => self.esqueleto("previa"),
            Some(Err(e)) => TermBlock::new().bad(e.clone()).into_any_element(),
            Some(Ok(EstadoPrevia::Recusada(b))) => bloco_da_recusa(b).into_any_element(),
            Some(Ok(EstadoPrevia::Pronta(p))) => {
                let mut coluna = div().flex().flex_col().gap(px(space::SM)).child(
                    Card::new()
                        .title(p.titulo.clone())
                        .description(p.descricao.clone())
                        .meta(p.id.clone()),
                );
                for s in &p.secoes {
                    coluna = coluna.child(cartao_da_secao(s));
                }
                coluna.into_any_element()
            }
        }
    }

    fn acoes(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let pronta = self.previa_pronta().is_some();
        let carregando = self.t_gerar.carregando();
        let (e_abrir, e_gerar, e_html) = (cx.entity(), cx.entity(), cx.entity());
        let mut abrir = Button::new("abrir-previa", "Abrir no navegador")
            .icon(IconName::Eye)
            .on_click(move |_, _, cx| e_abrir.update(cx, |v, cx| v.abrir_previa(cx)));
        let mut gerar = Button::new("gerar", "Gerar")
            .primary()
            .icon(IconName::Play)
            .loading(carregando)
            .on_click(move |_, _, cx| e_gerar.update(cx, |v, cx| v.gerar_pagina(false, cx)));
        let mut html = Button::new("gerar-html", "Gerar HTML")
            .icon(IconName::Download)
            .loading(carregando)
            .on_click(move |_, _, cx| e_html.update(cx, |v, cx| v.gerar_pagina(true, cx)));
        if !pronta {
            let motivo = "a prévia precisa estar pronta";
            abrir = abrir.disabled(motivo);
            gerar = gerar.disabled(motivo);
            html = html.disabled(motivo);
        }
        let mut linha = div()
            .flex()
            .flex_wrap()
            .gap(px(space::SM))
            .child(abrir)
            .child(gerar)
            .child(html);
        if let Some(comando) = self.rascunho.comando(None) {
            linha = linha.child(
                CopyButton::new("copiar-comando", comando)
                    .label("Copiar comando")
                    .copied_label("Copiado"),
            );
        }
        linha.into_any_element()
    }

    fn resultado(&self) -> gpui::AnyElement {
        match &self.gerado {
            None => div().into_any_element(),
            Some(Err(e)) => TermBlock::new().bad(e.clone()).into_any_element(),
            Some(Ok(Err(b))) => bloco_da_recusa(b).into_any_element(),
            Some(Ok(Ok(linhas))) => linhas
                .iter()
                .fold(TermBlock::new(), |t, l| t.line(l.clone()))
                .copy("copiar-gerado", linhas.join("\n"))
                .into_any_element(),
        }
    }
}

fn bloco_da_recusa(b: &Bloqueio) -> TermBlock {
    b.motivos
        .iter()
        .fold(TermBlock::new().bad(b.cabeca.clone()), |t, m| {
            t.line(format!("  {m}"))
        })
}

fn cartao_da_secao(s: &SecaoPrevia) -> Card {
    let mut bloco = TermBlock::new();
    for l in s.linhas.iter().take(LINHAS_POR_SECAO) {
        bloco = bloco.line(l.clone());
    }
    let resto = s.linhas.len().saturating_sub(LINHAS_POR_SECAO);
    if resto > 0 {
        bloco = bloco.dim(format!("e mais {resto}"));
    }
    Card::new()
        .title(s.titulo.clone())
        .meta(format!("{} itens", s.linhas.len()))
        .child(bloco)
}
