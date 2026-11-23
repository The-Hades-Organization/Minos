use super::*;
use std::sync::Arc;
use std::thread;

fn tmp(nome: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("minos-store-{nome}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn saida(file: &str) -> Saida {
    Saida {
        model: Modelo::Orquestracao,
        scope: Escopo::Pessoal,
        plan: "/tmp/plano.md".to_owned(),
        url: None,
        file: file.to_owned(),
        updated_at: "2026-01-02T03:04:05.000Z".to_owned(),
    }
}

fn grava(state: &Path, text: &str) {
    fs::create_dir_all(minos_dir(state)).unwrap();
    fs::write(registry_file(state), text).unwrap();
}

#[test]
fn id_de_saida() {
    assert!(is_output_id("orquestracao-plano"));
    assert!(is_output_id("9"));
    assert!(is_output_id(&"a".repeat(81)));
    assert!(!is_output_id(&"a".repeat(82)));
    assert!(!is_output_id("-a"));
    assert!(!is_output_id("A"));
    assert!(!is_output_id(""));
    assert!(!is_output_id("a\n"));
}

#[test]
fn id_derivado_da_fonte() {
    let o = Modelo::Orquestracao;
    assert_eq!(
        output_id(o, "/x/Plano Exemplo.MD", Escopo::Pessoal),
        "orquestracao-plano-exemplo"
    );
    assert_eq!(
        output_id(o, "/x/plano.md", Escopo::Time),
        "orquestracao-plano-team"
    );
    assert_eq!(
        output_id(Modelo::Proposta, "/x/--.json", Escopo::Pessoal),
        "proposta-plano"
    );
    assert_eq!(
        output_id(o, "ação_1.txt", Escopo::Pessoal),
        "orquestracao-a-o-1-txt"
    );
    let longo = format!("{}-b.md", "a".repeat(59));
    assert_eq!(
        output_id(o, &longo, Escopo::Pessoal),
        format!("orquestracao-{}-", "a".repeat(59))
    );
}

#[test]
fn url_do_artifact() {
    let ok = |raw: &str, esperado: &str| {
        assert_eq!(parse_artifact_url(raw).as_deref(), Some(esperado), "{raw}")
    };
    ok(
        "https://claude.ai/artifact/abc123",
        "https://claude.ai/artifact/abc123",
    );
    ok(
        "https://claude.ai/code/artifact/a.b_c-d/",
        "https://claude.ai/code/artifact/a.b_c-d",
    );
    ok(
        "https://CLAUDE.ai:443/artifact/x?q=1#f",
        "https://claude.ai/artifact/x",
    );
    ok(
        " https://claude.ai/x/../artifact/y",
        "https://claude.ai/artifact/y",
    );
    for raw in [
        "http://claude.ai/artifact/x",
        "https://exemplo.invalid/artifact/abc123",
        "https://claude.ai:8443/artifact/x",
        "https://u:p@claude.ai/artifact/x",
        "https://claude.ai/artifact/",
        "https://claude.ai/artifact/a/b",
        "https://claude.ai/chat/x",
        "nada",
    ] {
        assert_eq!(parse_artifact_url(raw), None, "{raw}");
    }
}

#[test]
fn leitura_descarta_invalido_em_silencio() {
    let state = tmp("leitura");
    grava(
        &state,
        r#"{"outputs":{
            "b":{"model":"orquestracao","scope":"team","plan":"p","url":"https://claude.ai/artifact/z/","file":"f","updatedAt":"t"},
            "10":{"model":"proposta","scope":"personal","plan":"p","url":"http://x","file":"f","updatedAt":"t"},
            "2":{"model":"explicacao","scope":"personal","plan":"p","url":7,"file":"f","updatedAt":"t"},
            "Ruim":{"model":"proposta","scope":"personal","plan":"p","file":"f","updatedAt":"t"},
            "c":{"model":"outro","scope":"personal","plan":"p","file":"f","updatedAt":"t"},
            "d":{"model":"proposta","scope":"todos","plan":"p","file":"f","updatedAt":"t"},
            "e":{"model":"proposta","scope":"personal","plan":"p","file":1,"updatedAt":"t"}
        }}"#,
    );
    let ids: Vec<_> = read_outputs(&state)
        .into_iter()
        .map(|(k, s)| (k, s.url))
        .collect();
    assert_eq!(
        ids,
        vec![
            ("2".to_owned(), None),
            ("10".to_owned(), None),
            (
                "b".to_owned(),
                Some("https://claude.ai/artifact/z".to_owned())
            ),
        ]
    );
    fs::remove_dir_all(&state).unwrap();
}

#[test]
fn leitura_tolerante() {
    let state = tmp("tolerante");
    assert!(read_outputs(&state).is_empty());
    for text in ["{", "[]", r#"{"outputs":null}"#, r#"{"outputs":"x"}"#] {
        grava(&state, text);
        assert!(read_outputs(&state).is_empty(), "{text}");
    }
    grava(
        &state,
        r#"{"outputs":[{"model":"proposta","scope":"personal","plan":"p","file":"f","updatedAt":"t"}]}"#,
    );
    assert_eq!(read_outputs(&state)[0].0, "0");
    fs::remove_dir_all(&state).unwrap();
}

#[test]
fn grava_no_formato_do_ts() {
    let state = tmp("formato");
    save_output(&state, "b", saida("fb")).unwrap();
    save_output(&state, "a", saida("fa")).unwrap();
    save_output(&state, "b", saida("fb2")).unwrap();
    let text = fs::read_to_string(registry_file(&state)).unwrap();
    assert!(
        text.starts_with("{\n  \"outputs\": {\n    \"b\": {\n      \"model\": \"orquestracao\",\n")
    );
    assert!(text.contains("\"url\": null,\n      \"file\": \"fb2\",\n      \"updatedAt\""));
    assert!(text.ends_with("}\n"));
    let ids: Vec<_> = read_outputs(&state).into_iter().map(|(k, _)| k).collect();
    assert_eq!(ids, ["b", "a"]);
    fs::remove_dir_all(&state).unwrap();
}

#[test]
fn link_grava_e_recusa() {
    let state = tmp("link");
    save_output(&state, "orquestracao-x", saida("f")).unwrap();
    assert!(matches!(
        gravar_link(
            &state,
            "orquestracao-x",
            "https://exemplo.invalid/artifact/a"
        ),
        Err(ErroLink::Url)
    ));
    assert!(matches!(
        gravar_link(&state, "nao-existe", "https://claude.ai/artifact/a"),
        Err(ErroLink::SaidaDesconhecida)
    ));
    assert!(matches!(
        gravar_link(&state, "X", "https://claude.ai/artifact/a"),
        Err(ErroLink::SaidaDesconhecida)
    ));
    let url = gravar_link(&state, "orquestracao-x", "https://claude.ai/artifact/a/").unwrap();
    assert_eq!(url, "https://claude.ai/artifact/a");
    assert_eq!(
        saidas(&state)[0].1.url.as_deref(),
        Some("https://claude.ai/artifact/a")
    );
    fs::remove_dir_all(&state).unwrap();
}

#[test]
fn gravacoes_simultaneas_nao_perdem_saida() {
    let state = Arc::new(tmp("concorrencia"));
    let handles: Vec<_> = (0..16)
        .map(|i| {
            let state = Arc::clone(&state);
            thread::spawn(move || save_output(&state, &format!("s{i}"), saida("f")).unwrap())
        })
        .collect();
    handles.into_iter().for_each(|h| h.join().unwrap());
    assert_eq!(read_outputs(&state).len(), 16);
    fs::remove_dir_all(&*state).unwrap();
}

#[test]
fn arquivos_da_saida() {
    let state = tmp("arquivos");
    let file = write_output_files(&state, "s", "<p>", "{}\n").unwrap();
    assert_eq!(file, minos_dir(&state).join("s/index.html"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "<p>");
    assert_eq!(
        fs::read_to_string(minos_dir(&state).join("s/data.json")).unwrap(),
        "{}\n"
    );
    fs::remove_dir_all(&state).unwrap();
}
