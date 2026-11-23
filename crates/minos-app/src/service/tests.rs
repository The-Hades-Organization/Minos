use std::path::Path;

use minos_core::store::save_output;
use minos_core::{Escopo, Modelo, Saida};

use super::*;

fn env_em(dir: &Path) -> Env {
    Env::empty()
        .with("HADES_DATA", dir.display().to_string())
        .with("HOME", dir.display().to_string())
}

fn saida(scope: Escopo) -> Saida {
    Saida {
        model: Modelo::Explicacao,
        scope,
        plan: "/demo/dados.json".to_owned(),
        url: None,
        file: "/demo/index.html".to_owned(),
        updated_at: "2026-11-19T09:12:30.000Z".to_owned(),
    }
}

#[test]
fn carrega_o_registro_e_a_flag_do_time() {
    let dir = tempfile::tempdir().expect("tmp");
    save_output(dir.path(), "explicacao-dados", saida(Escopo::Pessoal)).expect("grava");
    save_output(dir.path(), "explicacao-dados-team", saida(Escopo::Time)).expect("grava");
    let d = carregar(&env_em(dir.path()), dir.path()).expect("carrega");
    let ids: Vec<_> = d.saidas.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(ids, ["explicacao-dados", "explicacao-dados-team"]);
    assert!(!d.time_ligado);
    assert_eq!(d.pasta, dir.path().join("minos"));

    let ligado = env_em(dir.path()).with("HADES_MINOS_TEAM", "1");
    assert!(carregar(&ligado, dir.path()).expect("carrega").time_ligado);
}

#[test]
fn registro_vazio_nao_e_erro() {
    let dir = tempfile::tempdir().expect("tmp");
    let d = carregar(&env_em(dir.path()), dir.path()).expect("carrega");
    assert!(d.saidas.is_empty());
}

#[test]
fn grava_o_link_normalizado() {
    let dir = tempfile::tempdir().expect("tmp");
    let env = env_em(dir.path());
    save_output(dir.path(), "explicacao-dados", saida(Escopo::Pessoal)).expect("grava");
    let url = gravar(
        &env,
        dir.path(),
        "explicacao-dados",
        "https://claude.ai/artifact/abc?x=1#y",
    )
    .expect("grava link");
    assert_eq!(url, "https://claude.ai/artifact/abc");
    let d = carregar(&env, dir.path()).expect("carrega");
    assert_eq!(
        d.saidas[0].1.url.as_deref(),
        Some("https://claude.ai/artifact/abc")
    );
}

#[test]
fn recusas_do_link_com_o_texto_do_cli() {
    let dir = tempfile::tempdir().expect("tmp");
    let env = env_em(dir.path());
    save_output(dir.path(), "explicacao-dados", saida(Escopo::Pessoal)).expect("grava");
    assert_eq!(
        gravar(
            &env,
            dir.path(),
            "explicacao-dados",
            "https://exemplo.com/artifact/abc"
        ),
        Err(erro_de_url())
    );
    assert_eq!(
        gravar(
            &env,
            dir.path(),
            "nao-existe",
            "https://claude.ai/artifact/abc"
        ),
        Err(erro_de_saida("nao-existe"))
    );
    assert_eq!(
        gravar(
            &env,
            dir.path(),
            "Id Ruim",
            "https://claude.ai/artifact/abc"
        ),
        Err(erro_de_saida("Id Ruim"))
    );
}

#[test]
fn abrir_sem_abridor_no_path_avisa() {
    let env = Env::empty().with("PATH", "/nao/existe");
    assert_eq!(
        abrir(&env, "/demo/index.html"),
        Err("sem xdg-open nem open no PATH: abra /demo/index.html".to_owned())
    );
}

fn env_com_skill(dir: &Path) -> Env {
    let skill = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../parity/fixtures/skill-min");
    env_em(dir).with("HADES_SKILL_DIR", skill.display().to_string())
}

fn rascunho_explicacao(dir: &Path, dados: &serde_json::Value) -> Rascunho {
    let fonte = dir.join("dados.json");
    std::fs::write(&fonte, serde_json::to_string(dados).expect("json")).expect("grava");
    Rascunho {
        modelo: Modelo::Explicacao,
        fonte: Some(fonte),
        time: false,
    }
}

#[test]
fn previa_do_exemplo_fica_pronta_sem_gravar() {
    let dir = tempfile::tempdir().expect("tmp");
    let env = env_com_skill(dir.path());
    let exemplo = minos_core::modelo::exemplo(Modelo::Explicacao).expect("exemplo");
    let r = rascunho_explicacao(dir.path(), &exemplo);
    let EstadoPrevia::Pronta(p) = previa(&env, dir.path(), &r).expect("prévia") else {
        panic!("prévia recusada");
    };
    assert_eq!(p.id, "explicacao-dados");
    assert!(!p.secoes.is_empty());
    assert!(p.pagina.contains("<html"), "página sem html");
    assert!(
        carregar(&env, dir.path())
            .expect("carrega")
            .saidas
            .is_empty()
    );
}

#[test]
fn previa_de_json_invalido_e_recusada_com_a_cabeca_do_cli() {
    let dir = tempfile::tempdir().expect("tmp");
    let env = env_com_skill(dir.path());
    let fonte = dir.path().join("dados.json");
    std::fs::write(&fonte, "{ nada").expect("grava");
    let r = Rascunho {
        modelo: Modelo::Explicacao,
        fonte: Some(fonte),
        time: false,
    };
    let EstadoPrevia::Recusada(b) = previa(&env, dir.path(), &r).expect("prévia") else {
        panic!("prévia pronta");
    };
    assert_eq!(b.cabeca, "minos: recusado:");
    assert!(
        b.motivos[0].starts_with("dados não são JSON válido"),
        "{:?}",
        b.motivos
    );
}

#[test]
fn gerar_registra_a_saida_e_gerar_html_vai_para_offline() {
    let dir = tempfile::tempdir().expect("tmp");
    let env = env_com_skill(dir.path());
    let exemplo = minos_core::modelo::exemplo(Modelo::Explicacao).expect("exemplo");
    let r = rascunho_explicacao(dir.path(), &exemplo);

    let linhas = gerar(&env, dir.path(), &r, false)
        .expect("gera")
        .expect("liberado");
    assert!(linhas[0].starts_with("publique: "), "{linhas:?}");
    let d = carregar(&env, dir.path()).expect("carrega");
    assert_eq!(d.saidas.len(), 1);

    let linhas = gerar(&env, dir.path(), &r, true)
        .expect("gera")
        .expect("liberado");
    let esperado = dir.path().join("minos/offline/explicacao-dados.html");
    assert_eq!(linhas[0], format!("arquivo: {}", esperado.display()));
    assert!(esperado.exists());
}

#[test]
fn gerar_sem_fonte_pede_a_fonte() {
    let dir = tempfile::tempdir().expect("tmp");
    let env = env_com_skill(dir.path());
    let r = Rascunho::default();
    assert_eq!(
        gerar(&env, dir.path(), &r, true).unwrap_err(),
        "escolha a fonte"
    );
}
