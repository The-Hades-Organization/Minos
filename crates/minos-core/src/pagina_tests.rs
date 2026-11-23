//! Casos de `minos-modelo.test.ts` ("página") e os números de empate.

use super::*;
use crate::coleta::Entradas;
use crate::coleta::tests::{PLAN, agent, build, inputs};
use crate::modelo::{Estimativa, Limites};

const TEMPLATE: &str = include_str!("../../../parity/fixtures/skill-min/kit/minos/pagina.html");

fn orq(d: Orquestracao) -> Validado {
    Validado::Orquestracao(d)
}

fn hostile() -> Orquestracao {
    let mut a = agent();
    a.name = "<img src=x onerror=1>".into();
    a.open_questions = vec!["Q1 — </script><svg onload=1>".into()];
    build(Entradas {
        plan_text: PLAN.replace(
            "Acompanhar a entrega.",
            "<script>alert(1)</script> \"aspas\" & cia",
        ),
        agents: vec![a],
        ..inputs()
    })
}

#[test]
fn dado_entra_escapado_e_nada_vira_marcacao() {
    let html = render_page(TEMPLATE, &orq(hostile()));
    assert!(!html.contains("<script>alert"));
    assert!(!html.contains("<img src=x"));
    assert!(!html.contains("<svg onload"));
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt; &quot;aspas&quot; &amp; cia"));
    assert!(html.contains("Entrega &lt;b&gt;de exemplo&lt;/b&gt;"));
}

#[test]
fn unico_script_e_o_bloco_de_dados() {
    let data = hostile();
    let html = render_page(TEMPLATE, &orq(data.clone()));
    assert_eq!(html.matches("<script").count(), 1);
    assert!(html.contains("<script type=\"application/json\" id=\"minos-data\">"));
    let v = Validado::Orquestracao(data);
    let block = data_block(&v);
    assert!(!block.contains('<'));
    let back: serde_json::Value = serde_json::from_str(&block).unwrap();
    assert_eq!(json_compact(&back), json_compact(&v.to_value()));
}

#[test]
fn bloco_escapa_separadores_de_linha() {
    let mut a = agent();
    a.phase = "a\u{2028}b\u{2029}c".into();
    let v = Validado::Orquestracao(build(Entradas {
        agents: vec![a],
        ..inputs()
    }));
    let block = data_block(&v);
    assert!(block.contains("a\\u2028b\\u2029c"));
    assert!(!block.contains('\u{2028}') && !block.contains('\u{2029}'));
}

#[test]
fn autocontida_sem_recurso_externo_nem_evento_nem_marcador() {
    let clean = render_page(TEMPLATE, &orq(build(inputs()))).to_lowercase();
    assert!(!clean.contains("http://") && !clean.contains("https://"));
    for proibido in [
        "<link", "<img", "<iframe", "@import", "url(", " src=", " href=",
    ] {
        assert!(!clean.contains(proibido), "{proibido}");
    }
    let evento = Regex::new(r"\son[a-z]+=").unwrap();
    assert!(!evento.is_match(&clean));
    assert!(!Regex::new(r"\{\{[a-z]+\}\}").unwrap().is_match(&clean));
}

#[test]
fn tema_e_titulo_por_escopo() {
    let data = hostile();
    let html = render_page(TEMPLATE, &orq(data.clone()));
    assert!(html.contains("@media (prefers-color-scheme: dark)"));
    assert!(html.contains(":root:not([data-theme=\"light\"])"));
    assert!(html.contains(":root[data-theme=\"dark\"]"));
    assert_eq!(page_title(&data), "Orquestração Hades");
    let time = build(Entradas {
        scope: Escopo::Time,
        ..inputs()
    });
    assert_eq!(title_of(&orq(time.clone())), "Orquestração do time");
    assert!(html.contains("<title>Orquestração Hades</title>"));
    assert!(html.contains("<main class=\"ops\">"));
}

#[test]
fn marcador_dentro_de_valor_nao_e_relido() {
    let tricky = build(Entradas {
        plan_text: PLAN.replace("Acompanhar a entrega.", "{{DATA}}"),
        ..inputs()
    });
    assert!(render_page(TEMPLATE, &orq(tricky.clone())).contains("{{DATA}}"));
    assert_eq!(
        render_page("{{NADA}}|{{LAYOUT}}", &orq(tricky)),
        "{{NADA}}|ops"
    );
}

#[test]
fn esc_cobre_os_cinco_e_inline_troca_crase() {
    assert_eq!(esc("&<>\"'"), "&amp;&lt;&gt;&quot;&#39;");
    assert_eq!(
        inline("use `a<b` e `c`"),
        "use <code>a&lt;b</code> e <code>c</code>"
    );
    assert_eq!(inline("sem `fim"), "sem `fim");
}

#[test]
fn descricao_conta_ativos_perguntas_e_prs() {
    let data = build(inputs());
    assert_eq!(
        description_of(&orq(data.clone())),
        "Acompanhamento de Entrega <b>de exemplo</b>: 1 agente ativo, 1 pergunta aberta, 1 PR."
    );
    let vazio = build(Entradas {
        agents: vec![],
        ..inputs()
    });
    assert_eq!(
        description_of(&orq(vazio.clone())),
        "Acompanhamento de Entrega <b>de exemplo</b>: 0 agentes ativos, 0 perguntas abertas, 0 PRs."
    );
}

fn com_estimativa(cost: f64, minutes: f64, pct: f64) -> String {
    let mut data = build(Entradas {
        limits: Limites {
            five: Some(Limite {
                pct,
                resets_at: "2026-11-04T18:00:00Z".into(),
            }),
            seven: None,
        },
        ..inputs()
    });
    data.consumption.estimate = vec![Estimativa {
        unit: "api".into(),
        cost_p50: Some(cost),
        minutes_p50: Some(minutes),
        confidence: Confianca::Alta,
    }];
    render_orchestration_body(&data)
}

#[test]
fn numeros_com_empate_como_no_js() {
    let body = com_estimativa(0.125, 2.5, 23.5);
    assert!(body.contains("US$ 0.13"), "toFixed(2) de 0.125");
    assert!(body.contains("3 min"));
    assert!(body.contains("<span class=\"mono\">24%</span>"));
    assert!(body.contains("width:24%"));
    assert!(body.contains("renova 2026-11-04 18:00Z"));
    let body = com_estimativa(1.005, 0.5, 0.4);
    assert!(body.contains("US$ 1.00"), "1.005 é 1.00499… em f64");
    assert!(body.contains("1 min"));
    assert!(body.contains(">0%<"));
    assert_eq!(js_round(-2.5), -2.0);
    assert_eq!(js_round(0.49999999999999994), 0.0);
}

#[test]
fn corpo_sem_amostra_e_sem_estimativa() {
    let data = build(Entradas {
        plan_text: "# Demo".into(),
        agents: vec![],
        limits: Limites {
            five: None,
            seven: None,
        },
        ..inputs()
    });
    let body = render_orchestration_body(&data);
    assert!(body.contains("sem amostras"));
    assert!(body.contains("Plano sem estimativa da Moirai."));
    assert!(body.contains("Nenhum agente registrado."));
    assert!(body.contains("Plano sem &quot;Estado atual&quot;."));
    assert!(body.contains("nenhuma pergunta aberta · 0 agentes ativos · 0 PRs"));
    assert!(body.contains("atualizado em 2026-11-04 12:00Z"));
}
