//! `MinosView`: casca (trilho, rodapé) e as telas Publicações, Nova página, Página do time e Configurações.

mod tela_config;
mod tela_nova;

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, Styled, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use hades_core::Env;
use hades_ui::components::{
    Align, Button, Card, CopyButton, DataTable, EmptyState, MetaList, Page, PageHeader, Rail,
    RailBrand, RailGroup, RailItem, Segmented, Skeleton, StatusBadge, StatusBar, TermBlock,
    TextField,
};
use hades_ui::icons::IconName;
use hades_ui::palette;
use hades_ui::tokens::space;
use minos_core::{Modelo, Saida};

use crate::config::{self, ConfigLocal, FonteDeConfig};
use crate::nova::{EstadoPrevia, Rascunho};
use crate::service::{self, Dados, Gerado};
use crate::vm::{self, COLUNAS_PUBLICACOES, COLUNAS_TIME, Filtro, Linha, Tarefa, Tela};

pub struct MinosView {
    env: Env,
    cwd: PathBuf,
    tela: Tela,
    dados: Option<Dados>,
    aviso: Option<String>,
    filtro: Filtro,
    selecionada: Option<String>,
    foco: FocusHandle,
    recolhida: bool,
    t_dados: Tarefa,
    link: Option<Entity<InputState>>,
    erro_link: Option<String>,
    gravado: Option<String>,
    t_link: Tarefa,
    rascunho: Rascunho,
    previa: Option<Result<EstadoPrevia, String>>,
    t_previa: Tarefa,
    gerado: Option<Result<Gerado, String>>,
    t_gerar: Tarefa,
    fonte_config: Arc<dyn FonteDeConfig>,
    config: Option<Result<config::Linha, String>>,
    erro_config: Option<String>,
    t_config: Tarefa,
}

impl MinosView {
    pub fn new(env: Env, cwd: PathBuf, foco: FocusHandle, cx: &mut Context<Self>) -> Self {
        let fonte_config: Arc<dyn FonteDeConfig> = Arc::new(ConfigLocal::new(&env, &cwd));
        let mut view = Self {
            env,
            cwd,
            tela: Tela::Publicacoes,
            dados: None,
            aviso: None,
            filtro: Filtro::default(),
            selecionada: None,
            foco,
            recolhida: false,
            t_dados: Tarefa::default(),
            link: None,
            erro_link: None,
            gravado: None,
            t_link: Tarefa::default(),
            rascunho: Rascunho::default(),
            previa: None,
            t_previa: Tarefa::default(),
            gerado: None,
            t_gerar: Tarefa::default(),
            fonte_config,
            config: None,
            erro_config: None,
            t_config: Tarefa::default(),
        };
        view.recarregar(cx);
        view
    }

    /// Lê o registro e `HADES_MINOS_TEAM` no executor de fundo; resultado de uma geração velha é descartado.
    fn recarregar(&mut self, cx: &mut Context<Self>) {
        let Some(g) = self.t_dados.iniciar() else {
            return;
        };
        let (env, cwd) = (self.env.clone(), self.cwd.clone());
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_executor()
                .spawn(async move { service::carregar(&env, &cwd) })
                .await;
            let _ = this.update(cx, |v, cx| {
                if !v.t_dados.concluir(g) {
                    return;
                }
                match r {
                    Ok(d) => {
                        v.dados = Some(d);
                        v.aviso = None;
                    }
                    Err(e) => v.aviso = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn forcar_recarga(&mut self, cx: &mut Context<Self>) {
        self.t_dados.invalidar();
        self.recarregar(cx);
    }

    pub fn mudar_tela(&mut self, tela: Tela, cx: &mut Context<Self>) {
        self.tela = tela;
        self.forcar_recarga(cx);
        match tela {
            Tela::NovaPagina => self.recalcular_previa(cx),
            Tela::Configuracoes => self.carregar_config(cx),
            _ => {}
        }
    }

    fn saida(&self, id: &str) -> Option<&Saida> {
        let d = self.dados.as_ref()?;
        d.saidas.iter().find(|(k, _)| k == id).map(|(_, s)| s)
    }

    fn selecionar(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        if self.selecionada == id {
            return;
        }
        self.selecionada = id;
        self.erro_link = None;
        self.gravado = None;
        self.t_link.invalidar();
        self.link = None;
        cx.notify();
    }

    /// Cria o campo de colar link; precisa da janela.
    fn garantir_campo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.link.is_some() {
            return;
        }
        let e = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe(&e, |v: &mut MinosView, _, ev: &InputEvent, cx| match ev {
            InputEvent::Change => {
                v.erro_link = None;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => v.gravar_link(cx),
            _ => {}
        })
        .detach();
        self.link = Some(e);
    }

    /// Confere a URL como o `link` do CLI e grava no executor de fundo; recusa aparece no campo.
    fn gravar_link(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(campo)) = (self.selecionada.clone(), self.link.clone()) else {
            return;
        };
        let bruto = campo.read(cx).value().to_string();
        let url = match vm::conferir_link(&bruto) {
            Ok(Some(_)) => bruto.trim().to_owned(),
            Ok(None) => return,
            Err(e) => {
                self.erro_link = Some(e);
                cx.notify();
                return;
            }
        };
        let Some(g) = self.t_link.iniciar() else {
            return;
        };
        let (env, cwd) = (self.env.clone(), self.cwd.clone());
        cx.spawn(async move |this, cx| {
            let alvo = id.clone();
            let r = cx
                .background_executor()
                .spawn(async move { service::gravar(&env, &cwd, &alvo, &url) })
                .await;
            let _ = this.update(cx, |v, cx| {
                if !v.t_link.concluir(g) {
                    return;
                }
                match r {
                    Ok(url) => {
                        v.gravado = Some(format!("link de {id} gravado: {url}"));
                        v.erro_link = None;
                        v.link = None;
                        v.forcar_recarga(cx);
                    }
                    Err(e) => v.erro_link = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn abrir(&mut self, alvo: String, cx: &mut Context<Self>) {
        let env = self.env.clone();
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_executor()
                .spawn(async move { service::abrir(&env, &alvo) })
                .await;
            if let Err(e) = r {
                let _ = this.update(cx, |v, cx| {
                    v.aviso = Some(e);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn esqueleto(&self, id: &'static str) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(space::SM))
            .child(Skeleton::new(format!("{id}-1")))
            .child(Skeleton::new(format!("{id}-2")))
            .child(Skeleton::new(format!("{id}-3")))
            .into_any_element()
    }

    fn rodape(&self) -> (String, String) {
        let esquerda = self.aviso.clone().unwrap_or_else(|| {
            let Some(d) = &self.dados else {
                return if self.t_dados.carregando() {
                    "carregando…".to_owned()
                } else {
                    String::new()
                };
            };
            let sem_link = d.saidas.iter().filter(|(_, s)| s.url.is_none()).count();
            vm::rodape(d.saidas.len(), sem_link)
        });
        let direita = self
            .dados
            .as_ref()
            .map(|d| d.pasta.join("outputs.json").display().to_string())
            .unwrap_or_default();
        (esquerda, direita)
    }

    fn rail(&self, cx: &mut Context<Self>) -> Rail {
        let entidade = cx.entity();
        let recolhimento = cx.entity();
        let item = |t: Tela| RailItem::new(t.rotulo()).icon(t.icone());
        let mut publicacoes = item(Tela::Publicacoes);
        if let Some(d) = &self.dados {
            publicacoes = publicacoes.badge(d.saidas.len().to_string());
        }
        Rail::new([
            RailGroup::new(
                "Páginas",
                [publicacoes, item(Tela::NovaPagina), item(Tela::Time)],
            ),
            RailGroup::new("Sistema", [item(Tela::Configuracoes)]),
        ])
        .brand(
            RailBrand::new(crate::NOME)
                .icon(IconName::Scale)
                .subtitle("publicações"),
        )
        .theme_toggle()
        .collapsed(self.recolhida)
        .on_collapse(move |recolher, _, cx| {
            recolhimento.update(cx, |v, cx| {
                v.recolhida = recolher;
                cx.notify();
            });
        })
        .selected(self.tela.rotulo())
        .on_select(move |rotulo, _, cx| {
            if let Some(t) = Tela::de_rotulo(rotulo) {
                entidade.update(cx, |v, cx| {
                    v.mudar_tela(t, cx);
                    cx.notify();
                });
            }
        })
    }

    fn botao_recarregar(&self, cx: &mut Context<Self>) -> Button {
        let e = cx.entity();
        Button::new("recarregar", "Recarregar")
            .icon(IconName::RefreshCw)
            .loading(self.t_dados.carregando())
            .on_click(move |_, _, cx| e.update(cx, |v, cx| v.forcar_recarga(cx)))
    }

    fn tabela(
        &self,
        id: &'static str,
        colunas: &[&str],
        celulas: Vec<Vec<String>>,
        linhas: &[Linha],
        vazio: &'static str,
        cx: &mut Context<Self>,
    ) -> DataTable {
        let mut tabela = DataTable::new(
            colunas
                .iter()
                .map(|c| (SharedString::from(c.to_string()), Align::Start)),
        )
        .fill(id)
        .empty(vazio);
        for linha in celulas {
            tabela = tabela.row(linha.into_iter().map(SharedString::from));
        }
        if let Some(p) = vm::posicao_da_selecao(linhas, self.selecionada.as_deref()) {
            tabela = tabela.selected(p);
        }
        let entidade = cx.entity();
        let ids: Vec<String> = linhas.iter().map(|l| l.id.clone()).collect();
        tabela.on_select(move |p, _, cx| {
            let id = ids.get(p).cloned();
            entidade.update(cx, |v, cx| {
                v.selecionar(id, cx);
                cx.notify();
            });
        })
    }

    fn detalhe(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some((id, s)) = self
            .selecionada
            .as_deref()
            .and_then(|id| self.saida(id).map(|s| (id.to_owned(), s.clone())))
        else {
            return EmptyState::new("Nenhuma publicação selecionada")
                .hint("Escolha uma linha para abrir a página, copiar o comando ou colar o link.")
                .into_any_element();
        };
        let meta = MetaList::new()
            .label_width(96.0)
            .mono("Id", id.clone())
            .row("Modelo", s.model.title())
            .row("Escopo", vm::rotulo_do_escopo(s.scope))
            .mono("Fonte", s.plan.clone())
            .mono("Página", s.file.clone())
            .row("Atualizado", vm::quando(&s.updated_at))
            .row(
                "Link",
                s.url.clone().unwrap_or_else(|| vm::SEM_LINK.to_owned()),
            );
        let alvo = vm::alvo_de_abrir(&s);
        let e = cx.entity();
        let rotulo_abrir = if s.url.is_some() {
            "Abrir link"
        } else {
            "Abrir página"
        };
        let mut cartao = Card::new()
            .title(s.model.title())
            .status(StatusBadge::new(
                if s.url.is_some() {
                    "com link"
                } else {
                    vm::SEM_LINK
                },
                if s.url.is_some() {
                    hades_ui::Tone::Ok
                } else {
                    hades_ui::Tone::Neutral
                },
            ))
            .child(meta);
        if let Some(campo) = &self.link {
            cartao = cartao.child(
                TextField::new(campo)
                    .label("Link devolvido pela ferramenta Artifact")
                    .error_opt(self.erro_link.clone()),
            );
        }
        if let Some(g) = &self.gravado {
            cartao = cartao.child(TermBlock::new().line(g.clone()));
        }
        let gravar = cx.entity();
        cartao
            .action(
                Button::new("abrir", rotulo_abrir)
                    .icon(IconName::ExternalLink)
                    .on_click(move |_, _, cx| {
                        let alvo = alvo.clone();
                        e.update(cx, |v, cx| v.abrir(alvo, cx));
                    }),
            )
            .action(
                CopyButton::new("copiar-link", vm::comando_do_link(&id))
                    .label("Copiar comando")
                    .copied_label("Copiado"),
            )
            .action(
                Button::new("gravar-link", "Gravar link")
                    .primary()
                    .icon(IconName::Link)
                    .loading(self.t_link.carregando())
                    .on_click(move |_, _, cx| gravar.update(cx, |v, cx| v.gravar_link(cx))),
            )
            .into_any_element()
    }

    fn tela_publicacoes(&self, cx: &mut Context<Self>) -> Page {
        let entidade = cx.entity();
        let filtro = Segmented::new("filtro", Filtro::rotulos())
            .selected(self.filtro.indice())
            .on_select(move |i, _, cx| {
                entidade.update(cx, |v, cx| {
                    v.filtro = Filtro::de_indice(i);
                    cx.notify();
                });
            });
        let cabecalho = PageHeader::new("Publicações")
            .subtitle("As saídas preparadas e o link de cada uma")
            .description("O app não publica: a sessão publica com a ferramenta Artifact e grava o link aqui.")
            .action(self.botao_recarregar(cx));
        let pagina = Page::new("pagina-publicacoes").header(cabecalho);
        let Some(d) = &self.dados else {
            return pagina.child(self.esqueleto("publicacoes"));
        };
        if d.saidas.is_empty() {
            return pagina.child(
                EmptyState::new("Nenhuma publicação ainda")
                    .hint("hades minos publish orquestracao <plano.md>")
                    .fill(),
            );
        }
        let linhas = vm::linhas(&d.saidas, self.filtro);
        let celulas = linhas.iter().map(|l| l.celulas().to_vec()).collect();
        let tabela = self.tabela(
            "tabela-publicacoes",
            &COLUNAS_PUBLICACOES,
            celulas,
            &linhas,
            "Nenhuma publicação deste modelo",
            cx,
        );
        pagina
            .child(filtro)
            .fill_child(tabela)
            .child(self.detalhe(cx))
    }

    fn tela_time(&self, cx: &mut Context<Self>) -> Page {
        let cabecalho = PageHeader::new("Página do time")
            .subtitle("As publicações com escopo time")
            .action(self.botao_recarregar(cx));
        let pagina = Page::new("pagina-time").header(cabecalho);
        let Some(d) = &self.dados else {
            return pagina.child(self.esqueleto("time"));
        };
        let estado = vm::estado_do_time(d.time_ligado);
        let aviso = Card::new()
            .title("HADES_MINOS_TEAM")
            .status(StatusBadge::new(estado.rotulo, estado.tom))
            .child(TermBlock::new().warn(estado.aviso))
            .note(estado.dica);
        let linhas = vm::linhas_do_time(&d.saidas);
        let celulas = linhas
            .iter()
            .map(|l| l.celulas_do_time().to_vec())
            .collect();
        let tabela = self.tabela(
            "tabela-time",
            &COLUNAS_TIME,
            celulas,
            &linhas,
            "Nenhuma publicação com escopo time",
            cx,
        );
        pagina.child(aviso).fill_child(tabela)
    }

    fn campo_focado(&self, window: &Window, cx: &App) -> bool {
        self.link
            .as_ref()
            .is_some_and(|e| e.read(cx).focus_handle(cx).is_focused(window))
    }
}

impl Render for MinosView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.garantir_campo(window, cx);
        let p = palette(cx);
        let (esquerda, direita) = self.rodape();
        let rail = self.rail(cx);
        let corpo = match self.tela {
            Tela::Publicacoes => self.tela_publicacoes(cx).into_any_element(),
            Tela::Time => self.tela_time(cx).into_any_element(),
            Tela::NovaPagina => self.tela_nova(cx).into_any_element(),
            Tela::Configuracoes => self.tela_config(cx).into_any_element(),
        };
        div()
            .id("minos")
            .track_focus(&self.foco)
            .on_key_down(cx.listener(|v, ev: &KeyDownEvent, window, cx| {
                let campo_focado = v.campo_focado(window, cx);
                if let Some(t) = vm::tela_da_tecla(&ev.keystroke.key, campo_focado) {
                    v.mudar_tela(t, cx);
                    cx.notify();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.fg)
            .child(div().flex_1().flex().min_h_0().child(rail).child(corpo))
            .child(
                StatusBar::new(esquerda, direita)
                    .version(concat!("minos ", env!("CARGO_PKG_VERSION"))),
            )
    }
}

pub fn abrir_janela(cx: &mut App) {
    abrir_janela_em(cx, Tela::Publicacoes);
}

/// Abre a janela já na tela pedida (`--tela`).
pub fn abrir_janela_em(cx: &mut App, tela: Tela) {
    abrir_janela_com(
        cx,
        Abertura {
            tela,
            ..Abertura::default()
        },
    );
}

/// Estado inicial da janela vindo da linha de comando: tela, saída selecionada e rascunho da Nova página.
#[derive(Clone, Debug, Default)]
pub struct Abertura {
    pub tela: Tela,
    /// `--saida`: a publicação já selecionada, com o detalhe aberto.
    pub saida: Option<String>,
    /// `--modelo`: o modelo do rascunho da Nova página.
    pub modelo: Option<Modelo>,
    /// `--fonte`: a fonte do rascunho da Nova página; a prévia é calculada ao abrir a tela.
    pub fonte: Option<PathBuf>,
}

pub fn abrir_janela_com(cx: &mut App, abertura: Abertura) {
    let aberta = cx.open_window(gpui::WindowOptions::default(), |window, cx| {
        window.set_window_title(crate::NOME);
        let cwd = std::env::current_dir().unwrap_or_default();
        let view: Entity<MinosView> = cx.new(|cx| {
            let mut v = MinosView::new(Env::from_process(), cwd, cx.focus_handle(), cx);
            if let Some(m) = abertura.modelo {
                v.rascunho.modelo = m;
            }
            v.rascunho.fonte = abertura.fonte;
            v.selecionada = abertura.saida;
            if abertura.tela != Tela::Publicacoes {
                v.mudar_tela(abertura.tela, cx);
            }
            v
        });
        view
    });
    if aberta.is_err() {
        cx.quit();
    }
}
