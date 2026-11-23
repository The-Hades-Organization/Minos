use super::*;
use crate::documento::exemplo_explicacao;

const TEMPLATE: &str = include_str!("../../../parity/fixtures/skill-min/kit/minos/pagina.html");
const NOW_MS: i64 = 1_790_000_000_000;

fn tmp(nome: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("minos-publicar-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("skill/kit/minos")).unwrap();
    std::fs::write(dir.join("skill/kit/minos/pagina.html"), TEMPLATE).unwrap();
    dir
}

fn env(dir: &Path) -> Env {
    Env::empty()
        .with("HOME", dir.join("home").to_str().unwrap())
        .with("HADES_HOME", dir.join("hh").to_str().unwrap())
        .with("HADES_SKILL_DIR", dir.join("skill").to_str().unwrap())
}

fn sem_coletor(_: &Path, _: Escopo, _: i64) -> Result<Orquestracao, ErroColeta> {
    Err(ErroColeta::PlanoAusente(PathBuf::from("x")))
}

fn pedido(dir: &Path, model: Modelo, plan: &str) -> Pedido<'static> {
    Pedido {
        model,
        plan: plan.into(),
        scope: Escopo::Pessoal,
        html_file: None,
        state: dir.join("state"),
        team_enabled: true,
        credentials: None,
        skill_roots: vec![],
        cwd: dir.to_path_buf(),
        now_ms: NOW_MS,
        collect: &sem_coletor,
    }
}

fn recusa(r: Result<Publicado, Recusa>) -> (TipoRecusa, Vec<String>) {
    let e = r.unwrap_err();
    (e.kind, e.reasons)
}

#[test]
fn time_desligado_recusa_antes_de_tudo() {
    let dir = tmp("time");
    let mut p = pedido(&dir, Modelo::Explicacao, "nao-existe.json");
    p.scope = Escopo::Time;
    p.team_enabled = false;
    p.html_file = Some("x.txt".into());
    let (kind, reasons) = recusa(publish(&p, &env(&dir)));
    assert_eq!(kind, TipoRecusa::TeamOff);
    assert_eq!(reasons, TEAM_OFF.map(str::to_owned).to_vec());
}

#[test]
fn html_invalido_e_entrada() {
    let dir = tmp("html");
    let mut p = pedido(&dir, Modelo::Explicacao, "nao-existe.json");
    p.html_file = Some("x.txt".into());
    assert_eq!(
        recusa(publish(&p, &env(&dir))),
        (
            TipoRecusa::Input,
            vec!["--html: o arquivo precisa terminar em .html".into()]
        )
    );
    p.html_file = Some("falta/x.HTM".into());
    let (_, r) = recusa(publish(&p, &env(&dir)));
    assert_eq!(
        r,
        vec![format!(
            "--html: a pasta {} não existe",
            dir.join("falta").display()
        )]
    );
    std::fs::create_dir_all(dir.join("p.html")).unwrap();
    p.html_file = Some("p.html".into());
    let (_, r) = recusa(publish(&p, &env(&dir)));
    assert_eq!(
        r,
        vec![format!(
            "--html: {} é uma pasta",
            dir.join("p.html").display()
        )]
    );
}

#[test]
fn dados_ausentes_invalidos_e_fora_do_contrato() {
    let dir = tmp("dados");
    let e = env(&dir);
    let p = pedido(&dir, Modelo::Explicacao, "d.json");
    assert_eq!(
        recusa(publish(&p, &e)),
        (
            TipoRecusa::Input,
            vec!["dados não encontrados: d.json".into()]
        )
    );
    std::fs::write(dir.join("d.json"), "{x").unwrap();
    assert_eq!(
        recusa(publish(&p, &e)).1,
        vec!["dados não são JSON válido: d.json (o exemplo sai em hades minos schema explicacao)"]
    );
    std::fs::write(dir.join("d.json"), "[1]").unwrap();
    assert_eq!(
        recusa(publish(&p, &e)),
        (
            TipoRecusa::Contract,
            vec!["os dados não cumprem o contrato: $: esperava um objeto".into()]
        )
    );
    std::fs::write(dir.join("d.json"), "{\"model\":\"proposta\"}").unwrap();
    assert_eq!(recusa(publish(&p, &e)).0, TipoRecusa::Contract);
    std::fs::write(dir.join("d.json"), " ".repeat(DATA_FILE_MAX + 1)).unwrap();
    assert_eq!(
        recusa(publish(&p, &e)).1,
        vec![format!("dados acima de {DATA_FILE_MAX} bytes: d.json")]
    );
}

#[test]
fn coletor_ausente_e_entrada() {
    let dir = tmp("coletor");
    let p = pedido(&dir, Modelo::Orquestracao, "plano.md");
    assert_eq!(
        recusa(publish(&p, &env(&dir))),
        (TipoRecusa::Input, vec!["plano não encontrado: x".into()])
    );
}

#[test]
fn guarda_barra_sem_gravar_nada() {
    let dir = tmp("guarda");
    let mut doc = exemplo_explicacao();
    doc["summary"] = Value::String("veja /Users/fulano/notas".into());
    std::fs::write(dir.join("d.json"), doc.to_string()).unwrap();
    let mut p = pedido(&dir, Modelo::Explicacao, "d.json");
    p.html_file = Some("out.html".into());
    let (kind, reasons) = recusa(publish(&p, &env(&dir)));
    assert_eq!(kind, TipoRecusa::Guard);
    assert!(
        reasons.iter().any(|r| r.ends_with("  caminho da máquina")),
        "{reasons:?}"
    );
    assert!(!dir.join("out.html").exists());
    assert!(!dir.join("state").exists());
}

#[test]
fn escopo_vem_da_invocacao_e_generated_at_do_relogio() {
    let dir = tmp("escopo");
    let mut doc = exemplo_explicacao();
    doc["scope"] = Value::String("team".into());
    doc.as_object_mut().unwrap().remove("generatedAt");
    std::fs::write(dir.join("d.json"), doc.to_string()).unwrap();
    let Ok(Validado::Explicacao(v)) =
        load_document(Modelo::Explicacao, "d.json", &dir, Escopo::Pessoal, NOW_MS)
    else {
        panic!("devia validar");
    };
    assert_eq!(v.scope, Escopo::Pessoal);
    assert_eq!(v.generated_at, now_iso(NOW_MS));
    assert!(v.generated_at.ends_with('Z') && v.generated_at.len() == 20);
}
