use super::*;

fn saida(model: Modelo, scope: Escopo, url: Option<&str>) -> Saida {
    Saida {
        model,
        scope,
        plan: "/demo/acme/plano.md".to_owned(),
        url: url.map(str::to_owned),
        file: "/demo/estado/minos/x/index.html".to_owned(),
        updated_at: "2026-11-19T09:12:30.000Z".to_owned(),
    }
}

fn registro() -> Saidas {
    vec![
        (
            "orquestracao-plano".to_owned(),
            saida(Modelo::Orquestracao, Escopo::Pessoal, None),
        ),
        (
            "explicacao-dados".to_owned(),
            saida(
                Modelo::Explicacao,
                Escopo::Pessoal,
                Some("https://claude.ai/artifact/abc"),
            ),
        ),
        (
            "orquestracao-plano-team".to_owned(),
            saida(Modelo::Orquestracao, Escopo::Time, None),
        ),
    ]
}

#[test]
fn telas_na_ordem_do_trilho_e_por_nome() {
    let rotulos: Vec<_> = Tela::TODAS.iter().map(|t| t.rotulo()).collect();
    assert_eq!(
        rotulos,
        [
            "Publicações",
            "Nova página",
            "Página do time",
            "Configurações"
        ]
    );
    for t in Tela::TODAS {
        assert_eq!(Tela::de_rotulo(t.rotulo()), Some(t));
    }
    assert_eq!(Tela::de_nome("time"), Some(Tela::Time));
    assert_eq!(Tela::de_nome("nada"), None);
}

#[test]
fn tecla_troca_de_tela_so_sem_campo_focado() {
    assert_eq!(tela_da_tecla("1", false), Some(Tela::Publicacoes));
    assert_eq!(tela_da_tecla("4", false), Some(Tela::Configuracoes));
    assert_eq!(tela_da_tecla("5", false), None);
    assert_eq!(tela_da_tecla("0", false), None);
    assert_eq!(tela_da_tecla("a", false), None);
    assert_eq!(tela_da_tecla("1", true), None);
}

#[test]
fn tarefa_descarta_resultado_velho() {
    let mut t = Tarefa::default();
    let g = t.iniciar().expect("livre");
    assert!(t.carregando());
    assert_eq!(t.iniciar(), None);
    t.invalidar();
    assert!(!t.concluir(g));
    let g2 = t.iniciar().expect("livre depois de invalidar");
    assert!(t.concluir(g2));
    assert!(!t.carregando());
}

#[test]
fn filtro_por_modelo() {
    assert_eq!(
        Filtro::rotulos(),
        ["Todas", "Orquestração", "Explicação", "Proposta"]
    );
    let r = registro();
    assert_eq!(linhas(&r, Filtro::default()).len(), 3);
    let so_orq = Filtro::de_indice(1);
    assert_eq!(so_orq.indice(), 1);
    let ids: Vec<_> = linhas(&r, so_orq).into_iter().map(|l| l.id).collect();
    assert_eq!(ids, ["orquestracao-plano", "orquestracao-plano-team"]);
    assert!(linhas(&r, Filtro::de_indice(3)).is_empty());
    assert_eq!(Filtro::de_indice(9), Filtro::default());
}

#[test]
fn linha_mostra_escopo_fonte_data_e_link() {
    let r = registro();
    let l = linhas(&r, Filtro::default());
    assert_eq!(
        l[0].celulas(),
        [
            "orquestracao-plano",
            "Orquestração",
            "pessoal",
            "plano.md",
            "2026-11-19 09:12 UTC",
            "sem link",
        ]
        .map(str::to_owned)
    );
    assert_eq!(l[1].link, "https://claude.ai/artifact/abc");
    assert_eq!(l[2].escopo, "time");
}

#[test]
fn pagina_do_time_so_com_escopo_time() {
    let t = linhas_do_time(&registro());
    assert_eq!(t.len(), 1);
    assert_eq!(
        t[0].celulas_do_time(),
        [
            "orquestracao-plano-team",
            "Orquestração",
            "plano.md",
            "2026-11-19 09:12 UTC",
            "sem link",
        ]
        .map(str::to_owned)
    );
}

#[test]
fn data_fora_do_formato_passa_como_veio() {
    assert_eq!(quando("ontem"), "ontem");
    assert_eq!(
        quando("2026-11-19T09:12:30+03:00"),
        "2026-11-19T09:12:30+03:00"
    );
}

#[test]
fn nome_do_arquivo_da_fonte() {
    assert_eq!(nome_do_arquivo("/a/b/dados.json"), "dados.json");
    assert_eq!(nome_do_arquivo("plano.md"), "plano.md");
    assert_eq!(nome_do_arquivo("/a/b/"), "b");
}

#[test]
fn selecao_segue_o_id_entre_filtros() {
    let r = registro();
    let todas = linhas(&r, Filtro::default());
    assert_eq!(
        posicao_da_selecao(&todas, Some("orquestracao-plano-team")),
        Some(2)
    );
    let expl = linhas(&r, Filtro::de_indice(2));
    assert_eq!(
        posicao_da_selecao(&expl, Some("orquestracao-plano-team")),
        None
    );
    assert_eq!(posicao_da_selecao(&todas, None), None);
}

#[test]
fn comando_do_link_termina_em_espaco() {
    assert_eq!(
        comando_do_link("proposta-x"),
        "hades minos link proposta-x "
    );
}

#[test]
fn abrir_prefere_o_link_a_pagina_local() {
    let com = saida(
        Modelo::Proposta,
        Escopo::Pessoal,
        Some("https://claude.ai/artifact/z"),
    );
    assert_eq!(alvo_de_abrir(&com), "https://claude.ai/artifact/z");
    let sem = saida(Modelo::Proposta, Escopo::Pessoal, None);
    assert_eq!(alvo_de_abrir(&sem), "/demo/estado/minos/x/index.html");
}

#[test]
fn link_colado_valida_como_o_cli() {
    assert_eq!(conferir_link("  "), Ok(None));
    assert_eq!(
        conferir_link(" https://claude.ai/artifact/abc "),
        Ok(Some("https://claude.ai/artifact/abc".to_owned()))
    );
    assert_eq!(
        conferir_link("https://claude.ai/code/artifact/abc"),
        Ok(Some("https://claude.ai/code/artifact/abc".to_owned()))
    );
    for ruim in [
        "http://claude.ai/artifact/abc",
        "https://exemplo.com/artifact/abc",
        "https://claude.ai/chat/abc",
        "abc",
    ] {
        assert_eq!(
            conferir_link(ruim),
            Err("link: só https://claude.ai/artifact/<id> (ou /code/artifact/<id>)".to_owned()),
            "{ruim}"
        );
    }
    assert_eq!(
        erro_de_saida("x"),
        "saída desconhecida: x (hades minos list)"
    );
}

#[test]
fn time_so_liga_com_1_e_sempre_avisa_da_exposicao() {
    assert!(time_ligado("1"));
    for v in ["0", "", "true", "sim"] {
        assert!(!time_ligado(v), "{v}");
    }
    let off = estado_do_time(false);
    assert_eq!(off.rotulo, "desligada");
    assert_eq!(
        off.aviso,
        "A página do time está desligada (HADES_MINOS_TEAM=0): ela expõe o estado do trabalho a terceiros."
    );
    assert_eq!(
        off.dica,
        "Ligue no onboarding (passo \"Página do time\") ou em Configurações → Minos."
    );
    let on = estado_do_time(true);
    assert_eq!(on.tom, Tone::Warn);
    assert!(on.aviso.contains("expõe o estado do trabalho a terceiros"));
}

#[test]
fn rodape_conta_saidas_e_links() {
    assert_eq!(rodape(0, 0), "nenhuma saída ainda");
    assert_eq!(rodape(1, 1), "1 saída · 1 sem link");
    assert_eq!(rodape(3, 2), "3 saídas · 2 sem link");
}
