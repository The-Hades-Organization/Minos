use std::path::PathBuf;

use minos_core::documento::{exemplo_explicacao, exemplo_proposta};
use serde_json::json;

use super::*;

fn validado(m: Modelo, v: Value) -> Validado {
    let mut v = v;
    v["scope"] = json!("personal");
    v["generatedAt"] = json!("2026-11-19T09:12:30.000Z");
    m.validar(&v).expect("exemplo válido")
}

fn titulos(s: &[SecaoPrevia]) -> Vec<&str> {
    s.iter().map(|x| x.titulo.as_str()).collect()
}

#[test]
fn modelos_na_ordem_do_core_e_ida_e_volta_do_indice() {
    assert_eq!(
        rotulos_dos_modelos(),
        ["Orquestração", "Explicação de código", "Proposta"]
    );
    for (i, m) in modelos().iter().enumerate() {
        assert_eq!(modelo_de_indice(i), *m);
        assert_eq!(indice_do_modelo(*m), i);
    }
    assert_eq!(modelo_de_indice(99), Modelo::Explicacao);
}

#[test]
fn exemplo_so_para_quem_sai_de_json() {
    assert_eq!(exemplo_do_modelo(Modelo::Orquestracao), None);
    let ex = exemplo_do_modelo(Modelo::Proposta).expect("tem exemplo");
    assert!(ex.starts_with("{\n  \"model\": \"proposta\""));
    assert!(dica_da_fonte(Modelo::Orquestracao).contains(".md"));
    assert!(dica_da_fonte(Modelo::Explicacao).contains(".json"));
}

#[test]
fn rascunho_monta_o_comando_do_cli() {
    let mut r = Rascunho::default();
    assert_eq!(r.comando(None), None);
    assert_eq!(r.escopo(), Escopo::Pessoal);
    r.fonte = Some(PathBuf::from("/demo/dados.json"));
    assert_eq!(
        r.comando(None).as_deref(),
        Some("hades minos publish explicacao /demo/dados.json")
    );
    r.time = true;
    assert_eq!(r.escopo(), Escopo::Time);
    assert_eq!(
        r.comando(Some("/demo/p.html")).as_deref(),
        Some("hades minos publish explicacao /demo/dados.json --team --html /demo/p.html")
    );
}

#[test]
fn previa_da_explicacao_tem_titulo_descricao_e_secoes() {
    let d = validado(Modelo::Explicacao, exemplo_explicacao());
    let p = previa("explicacao-dados".into(), &d, "<html>".into());
    assert_eq!(p.titulo, "Como o pedido vira registro");
    assert_eq!(p.descricao, description_of(&d));
    assert_eq!(
        titulos(&p.secoes),
        [
            "Resumo",
            "Dados",
            "Fonte",
            "Fluxo",
            "Seções",
            "Arquivos",
            "Glossário",
            "Ressalvas",
            "Próximas perguntas"
        ]
    );
    let dados = &p.secoes[1].linhas;
    assert_eq!(
        dados,
        &["Pergunta: O que acontece entre a rota POST /pedidos e a gravação no banco?"]
    );
    assert_eq!(
        p.secoes[2].linhas,
        [
            "Repositório: dono/repo",
            "Referência: main@abc1234",
            "Caminhos: src/pedidos/"
        ]
    );
    assert_eq!(
        p.secoes[3].linhas[0],
        "A rota valida o corpo — Campo desconhecido volta 400 aqui."
    );
    assert_eq!(
        p.secoes[3].linhas[1],
        "O handler grava pedido e evento numa transação — src/pedidos/criar.handler.ts:18"
    );
    assert_eq!(
        p.secoes[5].linhas[0],
        "src/pedidos/rota.ts — entrada HTTP e validação"
    );
}

#[test]
fn previa_da_proposta_inclui_opcoes_e_recomendacao() {
    let d = validado(Modelo::Proposta, exemplo_proposta());
    let p = previa("proposta-x".into(), &d, String::new());
    let t = titulos(&p.secoes);
    for esperado in [
        "Resumo",
        "Dados",
        "Opções",
        "Recomendação",
        "Riscos",
        "Perguntas",
    ] {
        assert!(t.contains(&esperado), "faltou {esperado} em {t:?}");
    }
    let opcoes = p.secoes.iter().find(|s| s.titulo == "Opções").unwrap();
    assert_eq!(
        opcoes.linhas[0],
        "Nova tentativa com espera crescente — Três tentativas no próprio consumidor antes da fila de mortos."
    );
    let rec = p
        .secoes
        .iter()
        .find(|s| s.titulo == "Recomendação")
        .unwrap();
    assert_eq!(rec.linhas[0], "Opção: A");
}

#[test]
fn secao_longa_corta_com_e_mais() {
    let itens: Vec<Value> = (0..20).map(|i| json!(format!("item {i}"))).collect();
    let s = secoes(&json!({ "caveats": itens }));
    assert_eq!(s[0].linhas.len(), LINHAS_POR_SECAO + 1);
    assert_eq!(s[0].linhas.last().unwrap(), "e mais 8");
    assert!(secoes(&json!({ "caveats": [] })).is_empty());
    assert!(secoes(&json!([1])).is_empty());
}

#[test]
fn bloqueio_usa_a_cabeca_do_cli() {
    let guarda = Recusa {
        kind: TipoRecusa::Guard,
        reasons: vec!["página:3  nome de cliente".into()],
    };
    let b = bloqueio(&guarda);
    assert!(b.cabeca.starts_with("o Cerberus barrou a publicação"));
    assert_eq!(b.motivos, ["página:3  nome de cliente"]);
    let contrato = Recusa {
        kind: TipoRecusa::Contract,
        reasons: vec!["x".into()],
    };
    assert_eq!(bloqueio(&contrato).cabeca, "minos: recusado:");
}

#[test]
fn linhas_do_gerado_iguais_ao_cli() {
    let off = Publicado::Offline {
        file: PathBuf::from("/d/p.html"),
    };
    assert_eq!(
        linhas_do_gerado(&off),
        [
            "arquivo: /d/p.html",
            "Página autocontida (sem script nem recurso externo); nada saiu da máquina."
        ]
    );
    let art = Publicado::Artifact {
        id: "explicacao-dados".into(),
        file: PathBuf::from("/d/minos/explicacao-dados/index.html"),
        url: None,
        description: "desc".into(),
        scope: Escopo::Pessoal,
    };
    assert_eq!(
        linhas_do_gerado(&art),
        [
            "publique: /d/minos/explicacao-dados/index.html",
            "link registrado: nenhum (primeira publicação)",
            "descrição: desc",
            "depois: hades minos link explicacao-dados <url devolvida pela ferramenta Artifact>"
        ]
    );
}
