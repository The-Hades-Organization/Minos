use super::*;
use serde_json::json;

fn dict(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[test]
fn veredito_que_nao_barra_nao_conta() {
    assert!(barred_by(&dict(json!({"efeito": "avisa", "detalhe": "x"}))).is_empty());
}

#[test]
fn barra_com_conferido_da_evidencia() {
    let v = dict(
        json!({"efeito": "barra", "detalhe": "d", "evidencia": [{"conferido": "a"}, {"x": 1}, {"conferido": "b"}, 3]}),
    );
    assert_eq!(barred_by(&v), vec!["a", "b"]);
}

#[test]
fn barra_sem_conferido_cai_no_detalhe() {
    assert_eq!(
        barred_by(&dict(
            json!({"efeito": "barra", "detalhe": "d", "evidencia": []})
        )),
        vec!["d"]
    );
    assert_eq!(
        barred_by(&dict(json!({"efeito": "barra"}))),
        vec![SEM_DETALHE]
    );
}

#[test]
fn pasta_de_preparo_no_tmpdir() {
    let root = std::env::temp_dir().join(format!("minos-guarda-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let env = Env::empty().with("TMPDIR", format!("{}/", root.display()));
    let a = stage_dir(&env).unwrap();
    let b = stage_dir(&env).unwrap();
    assert_ne!(a, b);
    assert_eq!(a.parent(), Some(root.as_path()));
    assert!(
        a.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("minos-")
    );
    assert_eq!(tmp_root(&Env::empty()), PathBuf::from("/tmp"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn memoria_barra_antes_do_cerberus() {
    let env = Env::empty();
    let req = Pedido {
        page: "x\n/Users/fulano/y".into(),
        data: "{}".into(),
        ..Pedido::default()
    };
    assert_eq!(
        guard_publication(&req, &env).unwrap(),
        vec!["página:2  caminho da máquina"]
    );
}
