//! Casos de `loop/test/ts/minos-modelo.test.ts` ("contrato na fronteira"), com dados fictícios.

use super::*;
use hades_core::jsonfmt::json_compact;
use serde_json::json;

fn ok() -> Value {
    json!({
        "model": "orquestracao", "schema": 1, "scope": "personal", "generatedAt": "2026-11-04T12:00:00Z",
        "plan": {
            "title": "Entrega de exemplo", "objective": "Acompanhar a entrega.", "state": ["api — trabalhando"],
            "units": [
                { "name": "api", "role": "impl", "dependsOn": [] },
                { "name": "web", "role": "impl", "dependsOn": ["api"] },
            ],
        },
        "agents": [{
            "name": "api", "mode": "impl", "state": "working", "phase": "", "branch": "demo/api", "pr": 1234,
            "openQuestions": 1, "reportDone": false,
        }],
        "prs": [{ "number": 1234, "branch": "demo/api", "agent": "api" }],
        "escalations": [{ "agent": "api", "question": "Q1 — qual formato?" }],
        "consumption": {
            "estimate": [
                { "unit": "api", "costP50": 2.5, "minutesP50": 35, "confidence": "media" },
                { "unit": "web", "costP50": null, "minutesP50": null, "confidence": "nenhuma" },
            ],
            "limits": { "five": { "pct": 23.4, "resetsAt": "2026-11-04T18:00:00Z" }, "seven": null },
        },
    })
}

fn mutate(f: impl FnOnce(&mut Value)) -> Value {
    let mut d = ok();
    f(&mut d);
    d
}

fn refused(input: &Value) -> String {
    validar_orquestracao(input)
        .expect_err("o contrato aceitou o que devia recusar")
        .to_string()
}

#[test]
fn aceita_o_contrato_e_serializa_na_ordem_do_ts() {
    let d = validar_orquestracao(&ok()).unwrap();
    assert_eq!(d.agents[0].state, EstadoAgente::Working);
    assert_eq!(d.consumption.limits.seven, None);
    let v = Validado::Orquestracao(d).to_value();
    assert_eq!(json_compact(&v), json_compact(&ok()));
}

#[test]
fn ida_e_volta_em_json_da_o_mesmo() {
    let d = validar_orquestracao(&ok()).unwrap();
    let v = serde_json::to_value(&d).unwrap();
    assert_eq!(validar_orquestracao(&v).unwrap(), d);
}

#[test]
fn recusa_schema_de_outra_versao_modelo_errado_e_escopo_desconhecido() {
    assert_eq!(
        refused(&mutate(|d| d["schema"] = json!(2))),
        "schema: versão 2 não suportada (esta é a 1)"
    );
    assert_eq!(
        refused(&mutate(|d| d["schema"] = json!("1"))),
        "schema: versão 1 não suportada (esta é a 1)"
    );
    assert!(validar_orquestracao(&mutate(|d| d["schema"] = json!(1.0))).is_ok());
    assert_eq!(
        refused(&mutate(|d| d["model"] = json!("outro-modelo"))),
        "model: esperava \"orquestracao\""
    );
    assert_eq!(
        refused(&mutate(|d| d["scope"] = json!("publico"))),
        "scope: esperava um de personal, team"
    );
}

#[test]
fn recusa_tipo_errado_com_o_caminho_do_campo_sem_repetir_o_valor() {
    let m = refused(&mutate(|d| d["agents"][0]["pr"] = json!("doze")));
    assert!(m.contains("agents[0].pr") && !m.contains("doze"), "{m}");
    assert_eq!(
        refused(&mutate(|d| d["agents"][0]["reportDone"] = json!("sim"))),
        "agents[0].reportDone: esperava booleano"
    );
    let sem_pr = mutate(|d| {
        d["agents"][0].as_object_mut().unwrap().remove("pr");
    });
    assert_eq!(
        refused(&sem_pr),
        "agents[0].pr: esperava inteiro não negativo"
    );
    assert_eq!(
        validar_orquestracao(&mutate(|d| d["agents"][0]["pr"] = Value::Null))
            .unwrap()
            .agents[0]
            .pr,
        None
    );
}

#[test]
fn recusa_enum_instante_sem_z_texto_e_lista_acima_do_teto_e_pct_fora() {
    assert!(
        refused(&mutate(|d| d["agents"][0]["state"] = json!("dormindo")))
            .starts_with("agents[0].state:")
    );
    assert!(
        refused(&mutate(
            |d| d["generatedAt"] = json!("2026-11-04T12:00:00-03:00")
        ))
        .starts_with("generatedAt:")
    );
    assert!(
        refused(&mutate(|d| d["plan"]["title"] = json!("x".repeat(501))))
            .starts_with("plan.title:")
    );
    let prs: Vec<Value> = (0..201)
        .map(|i| json!({ "number": i, "branch": "b", "agent": "a" }))
        .collect();
    assert_eq!(
        refused(&mutate(|d| d["prs"] = json!(prs))),
        "prs: lista acima de 200 itens"
    );
    assert_eq!(
        refused(&mutate(
            |d| d["consumption"]["limits"]["five"]["pct"] = json!(101)
        )),
        "consumption.limits.five.pct: esperava número de 0 a 100"
    );
    assert_eq!(
        refused(&mutate(
            |d| d["consumption"]["limits"]["five"]["pct"] = Value::Null
        )),
        "consumption.limits.five.pct: esperava número de 0 a 100"
    );
    assert_eq!(
        refused(&mutate(
            |d| d["consumption"]["estimate"][0]["confidence"] = json!("total")
        )),
        "consumption.estimate[0].confidence: esperava um de alta, media, baixa, nenhuma"
    );
}

#[test]
fn recusa_o_que_nao_e_objeto_e_ignora_campo_extra() {
    assert_eq!(refused(&Value::Null), "$: esperava um objeto");
    assert_eq!(
        refused(&mutate(|d| d["plan"] = json!([]))),
        "plan: esperava um objeto"
    );
    let extra = validar_orquestracao(&mutate(|d| d["novo"] = json!(1))).unwrap();
    assert!(serde_json::to_value(extra).unwrap().get("novo").is_none());
}

#[test]
fn ordem_das_recusas_segue_o_ts() {
    // plan e consumption são conferidos antes de scope
    let d = mutate(|d| {
        d["scope"] = json!("publico");
        d["consumption"] = json!(null);
    });
    assert!(refused(&d).starts_with("consumption:"));
    // em cada agente, pr e reportDone antes de name
    let d = mutate(|d| {
        d["agents"][0]["name"] = json!(1);
        d["agents"][0]["reportDone"] = json!(null);
    });
    assert!(refused(&d).starts_with("agents[0].reportDone:"));
}

#[test]
fn modelos_na_ordem_do_ts_com_origem_e_exemplo() {
    let ids: Vec<_> = modelos().iter().map(|m| m.id()).collect();
    assert_eq!(ids, ["orquestracao", "explicacao", "proposta"]);
    assert_eq!(Modelo::from_id("proposta"), Some(Modelo::Proposta));
    assert_eq!(Modelo::from_id("lote-de-reviews"), None);
    assert_eq!(Modelo::Orquestracao.origem().as_str(), "plan");
    assert_eq!(Modelo::Explicacao.origem(), Origem::Json);
    assert_eq!(exemplo(Modelo::Orquestracao), None);
    assert!(exemplo(Modelo::Proposta).is_some());
    assert_eq!(Modelo::Explicacao.title(), "Explicação de código");
}

#[test]
fn validar_despacha_pelo_modelo() {
    let v = Modelo::Orquestracao.validar(&ok()).unwrap();
    assert_eq!(v.modelo(), Modelo::Orquestracao);
    assert_eq!(
        Modelo::Proposta.validar(&ok()).unwrap_err().to_string(),
        "model: esperava \"proposta\""
    );
}
