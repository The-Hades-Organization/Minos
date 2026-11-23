//! Casos de `minos-modelo.test.ts` ("dados") e do parser do plano.

use super::*;
use crate::modelo::{EstadoAgente, Validado};

pub(crate) const PLAN: &str = "# Plano — Entrega <b>de exemplo</b>

## Estado atual
- **Unidades:** api — trabalhando
- **Aguardando:** o dono

## Objetivo
Acompanhar a entrega.

## Unidades
| Unidade | Card | Papel | Modelo/esforço | Base | Depende de | Specs |
|---|---|---|---|---|---|---|
| api | novo | impl | x | main | — | S1 |
| web | novo | impl | x | main | `api` | S2 |

## Estimativa
| Unidade | Custo p25 (US$) | Custo mediana (US$) | Custo p75 (US$) | Duração p25 (min) | Duração mediana (min) | Duração p75 (min) | Gate mediana (min) | Amostra | Confiança | Base |
|---|---|---|---|---|---|---|---|---|---|---|
| api | 1 | 2.5 | 4 | 10 | 35 | 60 | 5 | 4 | media | parecidas |
| web | — | — | — | — | — | — | — | 0 | nenhuma | sem-historico |
";

/// 2026-11-04T12:00:00Z.
pub(crate) const NOW_MS: i64 = 1_793_793_600_000;

pub(crate) fn agent() -> FonteAgente {
    FonteAgente {
        name: "api".into(),
        mode: "impl".into(),
        state: "working".into(),
        phase: String::new(),
        branch: "skill/api".into(),
        pr: Some(12),
        open_questions: vec!["Q1 — qual formato?".into()],
        report_done: false,
    }
}

pub(crate) fn inputs() -> Entradas {
    Entradas {
        plan_text: PLAN.into(),
        agents: vec![agent()],
        limits: Limites {
            five: Some(Limite {
                pct: 23.4,
                resets_at: "2026-11-04T18:00:00Z".into(),
            }),
            seven: None,
        },
        scope: Escopo::Pessoal,
        now_ms: NOW_MS,
    }
}

pub(crate) fn build(input: Entradas) -> Orquestracao {
    build_orchestration(&input).expect("dados válidos")
}

#[test]
fn monta_contrato_valido_e_completo() {
    let data = build(inputs());
    let ida = Validado::Orquestracao(data.clone()).to_value();
    assert_eq!(validar_orquestracao(&ida).unwrap(), data);
    assert_eq!(data.schema, ORQUESTRACAO_SCHEMA);
    assert_eq!(data.generated_at, "2026-11-04T12:00:00Z");
    assert_eq!(data.plan.title, "Entrega <b>de exemplo</b>");
    let units: Vec<_> = data
        .plan
        .units
        .iter()
        .map(|u| (u.name.as_str(), u.depends_on.clone()))
        .collect();
    assert_eq!(
        units,
        vec![("api", vec![]), ("web", vec!["api".to_owned()])]
    );
    assert_eq!(
        data.plan.state,
        ["Unidades: api — trabalhando", "Aguardando: o dono"]
    );
    assert_eq!(data.plan.objective, "Acompanhar a entrega.");
    assert_eq!(data.prs.len(), 1);
    assert_eq!(
        (
            data.prs[0].number,
            data.prs[0].branch.as_str(),
            data.prs[0].agent.as_str()
        ),
        (12, "skill/api", "api")
    );
    assert_eq!(data.escalations.len(), 1);
    assert_eq!(data.escalations[0].question, "Q1 — qual formato?");
    assert_eq!(data.agents[0].open_questions, 1);
}

#[test]
fn estimativa_le_medianas_e_traco_vira_null() {
    let e = parse_plan(PLAN).estimate;
    assert_eq!(e.len(), 2);
    assert_eq!(
        (
            e[0].unit.as_str(),
            e[0].cost_p50,
            e[0].minutes_p50,
            e[0].confidence
        ),
        ("api", Some(2.5), Some(35.0), Confianca::Media)
    );
    assert_eq!(
        (
            e[1].unit.as_str(),
            e[1].cost_p50,
            e[1].minutes_p50,
            e[1].confidence
        ),
        ("web", None, None, Confianca::Nenhuma)
    );
}

#[test]
fn plano_vazio_e_nenhum_agente_dao_listas_vazias_e_null() {
    let data = build(Entradas {
        plan_text: String::new(),
        agents: vec![],
        limits: Limites {
            five: None,
            seven: None,
        },
        ..inputs()
    });
    assert_eq!(data.plan.title, "Orquestração");
    assert_eq!(data.plan.objective, "");
    assert!(data.plan.state.is_empty() && data.plan.units.is_empty());
    assert!(data.agents.is_empty() && data.prs.is_empty() && data.escalations.is_empty());
    assert!(data.consumption.estimate.is_empty());
    assert_eq!(data.consumption.limits.five, None);
    assert_eq!(data.consumption.limits.seven, None);
}

#[test]
fn estado_desconhecido_vira_unknown_e_pr_conta_uma_vez() {
    let mut voando = agent();
    voando.state = "voando".into();
    let mut web = agent();
    web.name = "web".into();
    let data = build(Entradas {
        agents: vec![voando, web],
        ..inputs()
    });
    assert_eq!(data.agents[0].state, EstadoAgente::Unknown);
    assert_eq!(data.prs.len(), 1);
    assert_eq!(data.escalations.len(), 2);
}

#[test]
fn secao_ignora_cabecalho_dentro_de_bloco_cercado() {
    let plan = "## Objetivo\nantes\n```\n## Falso\n```\ndepois\n## Outra\nfora";
    assert_eq!(
        section_lines(plan, "Objetivo"),
        ["antes", "```", "## Falso", "```", "depois"]
    );
    assert!(section_lines(plan, "Ausente").is_empty());
    // o título precisa terminar em fronteira: `## Objetivos` não é `## Objetivo`
    assert!(section_lines("## Objetivos\nx", "Objetivo").is_empty());
    assert_eq!(
        section_lines("## Objetivo:\r\nx\r\n", "Objetivo"),
        ["x", ""]
    );
}

#[test]
fn corte_em_480_com_reticencias() {
    let long = "a".repeat(500);
    let c = cap(&long);
    assert_eq!(js_len(&c), 480);
    assert!(c.ends_with('…'));
    assert_eq!(cap(&"b".repeat(480)), "b".repeat(480));
    let plan = format!("# {long}\n## Objetivo\n{long}\n## Estado atual\n- {long}");
    let p = parse_plan(&plan);
    assert_eq!(js_len(&p.title), 480);
    assert_eq!(js_len(&p.objective), 480);
    assert_eq!(js_len(&p.state[0]), 480);
}

#[test]
fn titulo_sem_prefixo_e_sem_titulo_vira_padrao() {
    assert_eq!(parse_plan("# Plano - Demo").title, "Demo");
    assert_eq!(parse_plan("# PLANO – Demo").title, "Demo");
    assert_eq!(parse_plan("# Planos novos").title, "Planos novos");
    assert_eq!(parse_plan("texto\n#   Segundo  \n").title, "Segundo");
    assert_eq!(parse_plan("## Só seção").title, "Orquestração");
    assert_eq!(parse_plan("#sem espaço").title, "Orquestração");
    assert_eq!(parse_plan("#  ").title, "Orquestração");
}

#[test]
fn unidades_recusam_nome_invalido_e_dependencias_vazias() {
    let plan = "## Unidades\n| Unidade | Papel | Depende de |\n|:--|--|--:|\n| API | x | — |\n| ok-1 | `p` | a; n/a, Nenhuma ,b |\n| | y | |";
    let u = parse_plan(plan).units;
    assert_eq!(u.len(), 1);
    assert_eq!(u[0].name, "ok-1");
    assert_eq!(u[0].role, "p");
    assert_eq!(u[0].depends_on, ["a", "b"]);
}

#[test]
fn numeros_da_estimativa_seguem_o_number_do_js() {
    for (cell, want) in [
        ("2,5", Some(2.5)),
        (" 0x10 ", Some(16.0)),
        ("1e2", Some(100.0)),
        (".5", Some(0.5)),
        ("5.", Some(5.0)),
        ("-1", None),
        ("", None),
        ("  ", None),
        ("abc", None),
        ("inf", None),
        ("Infinity", None),
        ("1e400", None),
        ("1,5,0", None),
    ] {
        assert_eq!(number_of(cell), want, "{cell:?}");
    }
}

#[test]
fn confianca_desconhecida_vira_nenhuma() {
    let plan = "## Estimativa\n| Unidade | Confiança |\n|--|--|\n| a | ALTA |\n| b | talvez |";
    let e = parse_plan(plan).estimate;
    assert_eq!(e[0].confidence, Confianca::Alta);
    assert_eq!(e[1].confidence, Confianca::Nenhuma);
}

#[test]
fn pr_so_inteiro_positivo() {
    use serde_json::json;
    assert_eq!(pr_number(Some(&json!(12))), Some(12));
    assert_eq!(pr_number(Some(&json!(12.0))), Some(12));
    assert_eq!(pr_number(Some(&json!(1.5))), None);
    assert_eq!(pr_number(Some(&json!(0))), None);
    assert_eq!(pr_number(Some(&json!("12"))), None);
    assert_eq!(pr_number(None), None);
}

#[test]
fn coleta_le_registro_perguntas_e_relatorio() {
    let dir = std::env::temp_dir().join(format!("minos-coleta-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let orch = dir.join("orch");
    let wt = dir.join("wt");
    std::fs::create_dir_all(orch.join("agents")).unwrap();
    std::fs::create_dir_all(wt.join(".design-sync-reports")).unwrap();
    std::fs::write(dir.join("plan.md"), PLAN).unwrap();
    let record = serde_json::json!({
        "name": "api", "mode": "impl", "state": "idle", "branch": "demo/api", "pr": 1234,
        "worktree": wt.to_str().unwrap(),
    });
    std::fs::write(orch.join("agents/api.json"), record.to_string()).unwrap();
    std::fs::write(orch.join("agents/sem-nome.json"), "{\"mode\":\"x\"}").unwrap();
    std::fs::write(orch.join("agents/quebrado.json"), "{").unwrap();
    std::fs::write(
        wt.join(".design-sync-reports/questions.md"),
        "## Q1 — formato?\n## Q2 — prazo?\n## Resposta do dono (Q2)\nsim\n",
    )
    .unwrap();
    std::fs::write(
        wt.join(".design-sync-reports/report.md"),
        "\n STATUS: DONE\n",
    )
    .unwrap();
    let work = WorkEnv {
        skill_scope: true,
        repo_top: dir.clone(),
        main_clone: dir.clone(),
        slug: "demo".into(),
        repo_name: "demo".into(),
        work: dir.clone(),
        locks: dir.join("locks"),
        state: dir.join("state"),
        orch: orch.clone(),
        skill_orch: dir.join("skill"),
        reports: DEFAULT_REPORTS.into(),
    };
    let env = Env::empty()
        .with("HOME", dir.join("home").to_str().unwrap())
        .with("HADES_HOME", dir.join("hh").to_str().unwrap())
        .with("HADES_SKILL_DIR", dir.join("skill-dir").to_str().unwrap());
    let data = collect(
        &dir.join("plan.md"),
        Escopo::Time,
        NOW_MS,
        &work,
        &env,
        &dir,
    )
    .unwrap();
    assert_eq!(data.agents.len(), 1);
    let a = &data.agents[0];
    assert_eq!(
        (a.state, a.pr, a.open_questions, a.report_done),
        (EstadoAgente::Idle, Some(1234), 1, true)
    );
    assert_eq!(data.escalations[0].question, "Q1 — formato?");
    assert_eq!(data.consumption.limits.five, None);

    let ausente = collect(
        &dir.join("nada.md"),
        Escopo::Time,
        NOW_MS,
        &work,
        &env,
        &dir,
    )
    .unwrap_err();
    assert!(ausente.to_string().starts_with("plano não encontrado: "));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn janela_atual_le_a_ultima_amostra_aberta() {
    let dir = std::env::temp_dir().join(format!("minos-janela-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("home/.claude/hades/limits")).unwrap();
    let reset = NOW_MS / 1000 + 3600;
    let lines = [
        format!("{{\"at\":\"2026-11-04T10:00:00Z\",\"five\":{{\"pct\":10,\"resetsAt\":{reset}}}}}"),
        format!(
            "{{\"at\":\"2026-11-04T11:00:00Z\",\"five\":{{\"pct\":150.5,\"resetsAt\":{reset}.9}}}}"
        ),
    ];
    std::fs::write(
        dir.join("home/.claude/hades/limits/samples.jsonl"),
        lines.join("\n"),
    )
    .unwrap();
    let env = Env::empty()
        .with("HOME", dir.join("home").to_str().unwrap())
        .with("HADES_HOME", dir.join("hh").to_str().unwrap())
        .with("HADES_SKILL_DIR", dir.join("skill-dir").to_str().unwrap());
    let five = janela_atual(Janela::Five, NOW_MS, &env, &dir).unwrap();
    assert_eq!(five.pct, 100.0);
    assert_eq!(five.resets_at, "2026-11-04T13:00:00Z");
    assert_eq!(janela_atual(Janela::Seven, NOW_MS, &env, &dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}
