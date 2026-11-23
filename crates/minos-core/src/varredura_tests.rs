use super::*;

fn texto(label: &str, text: &str) -> Texto {
    Texto {
        label: label.to_owned(),
        text: text.to_owned(),
    }
}

fn termos() -> GuardTerms {
    GuardTerms {
        names: vec!["projeto-x".into()],
        vocab: vec!["fluxo(?:-a)?".into()],
        domain: vec!["dominio\\.teste".into()],
        allow: vec![],
        slug: String::new(),
    }
}

fn tipos(texts: &[Texto], terms: &GuardTerms) -> Vec<String> {
    scan_texts(texts, terms)
        .unwrap()
        .iter()
        .map(format_finding)
        .collect()
}

#[test]
fn texto_limpo_nao_gera_achado() {
    assert!(tipos(&[texto("página", "nada aqui\noutra linha")], &termos()).is_empty());
}

#[test]
fn cada_tipo_com_rotulo_e_linha() {
    let page = [
        "ok",
        "/Users/fulano/x",
        "fale com ana@empresa.com.br",
        "o projeto-x saiu",
        "Fluxo-A aberto",
        "em dominio.teste",
    ]
    .join("\n");
    assert_eq!(
        tipos(&[texto("página", &page)], &termos()),
        vec![
            "página:2  caminho da máquina",
            "página:3  e-mail",
            "página:4  nome de projeto",
            "página:5  nome de projeto",
            "página:6  domínio de projeto",
        ]
    );
}

#[test]
fn credencial_vence_os_outros_tipos() {
    let line = format!(
        "{}{} em /Users/fulano/",
        "gh", "p_abcdefghijklmnopqrstuvwxyz0123456789"
    );
    assert_eq!(
        tipos(&[texto("dados", &line)], &termos()),
        vec!["dados:1  credencial"]
    );
    let env_line = format!("{}={}", "JWT_SECRET", "abcdefgh12");
    assert_eq!(
        tipos(&[texto("dados", &env_line)], &termos()),
        vec!["dados:1  credencial"]
    );
}

#[test]
fn email_neutro_passa() {
    assert!(tipos(&[texto("p", "noreply@x.org e a@example.com")], &termos()).is_empty());
}

#[test]
fn slug_isenta_nome_mas_nao_dominio() {
    let mut t = termos();
    t.slug = "Org/Projeto-X-Skill".into();
    assert!(tipos(&[texto("p", "veja org/projeto-x-skill")], &t).is_empty());
    assert_eq!(
        tipos(&[texto("p", "org/projeto-x-skill em dominio.teste")], &t),
        vec!["p:1  domínio de projeto"]
    );
}

#[test]
fn termo_que_nao_compila_vira_erro() {
    let mut t = termos();
    t.vocab = vec!["(".into()];
    let err = scan_texts(&[texto("p", "x")], &t).unwrap_err();
    assert!(
        err.starts_with("termo de vocabulário não compila ((): "),
        "{err}"
    );
    let mut t = termos();
    t.domain = vec!["[".into()];
    assert!(
        scan_texts(&[texto("p", "x")], &t)
            .unwrap_err()
            .starts_with("termo de domínio não compila")
    );
}

#[test]
fn nome_entra_escapado() {
    let mut t = termos();
    t.names = vec!["a.b".into()];
    assert!(tipos(&[texto("p", "axb")], &t).is_empty());
    assert_eq!(
        tipos(&[texto("p", "A.B")], &t),
        vec!["p:1  nome de projeto"]
    );
}

#[test]
fn valores_de_credencial() {
    let md = "# título\n- `CHAVE_A` = \"valor-longo-1\"\nCURTA=1234567\nB: 'abcdefgh'\nminuscula=abcdefghij\n";
    assert_eq!(
        credential_values(Some(md)),
        vec!["valor-longo-1", "abcdefgh"]
    );
    assert!(credential_values(None).is_empty());
}

#[test]
fn varredura_soma_valor_do_credenciais() {
    let md = "SEGREDO=valor-longo-1";
    let out = scan_output(
        &[
            texto("página", "x\nvem valor-longo-1 aqui"),
            texto("dados", "ok"),
        ],
        Some(md),
        &termos(),
    )
    .unwrap();
    assert_eq!(out, vec!["página:2  valor do credenciais.md"]);
}

#[test]
fn slug_lido_do_config_do_kit() {
    let dir = std::env::temp_dir().join(format!("minos-varredura-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("kit")).unwrap();
    std::fs::write(
        dir.join("kit/config.tsv"),
        "OUTRA\tx\nHADES_VERSION_CHECK_REPO\tOrg/Repo\n",
    )
    .unwrap();
    assert_eq!(
        skill_repo_slug(&[PathBuf::from("/nao/existe"), dir.clone()]),
        "Org/Repo"
    );
    assert_eq!(skill_repo_slug(&[]), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn identidade_tem_segredos_e_usuario() {
    let p = identity_pattern();
    assert!(p.contains("devtunnels"));
    assert!(p.contains("/home/"));
    assert!(Regex::new(&p).is_ok());
}
