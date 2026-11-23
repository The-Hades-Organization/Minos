//! Casos de `loop/test/ts/minos-documento.test.ts` ("contrato") e do `schema` da CLI.

use super::*;
use crate::modelo::exemplo;
use hades_core::jsonfmt::json_indent;

fn com_cabecalho(mut v: Value, over: Value) -> Value {
    let o = v.as_object_mut().unwrap();
    o.insert("scope".into(), json!("personal"));
    o.insert("generatedAt".into(), json!("2026-11-09T12:00:00Z"));
    for (k, x) in over.as_object().unwrap() {
        o.insert(k.clone(), x.clone());
    }
    v
}

fn prop(over: Value) -> Value {
    com_cabecalho(exemplo_proposta(), over)
}

fn expl(over: Value) -> Value {
    com_cabecalho(exemplo_explicacao(), over)
}

fn refused<T: std::fmt::Debug>(r: Resultado<T>) -> String {
    r.expect_err("o contrato aceitou o que devia recusar")
        .to_string()
}

#[test]
fn exemplo_igual_byte_a_byte_ao_do_ts() {
    let e = json_indent(&exemplo(Modelo::Explicacao).unwrap(), 2);
    assert_eq!(
        e,
        include_str!("../fixtures/explicacao.schema.json").trim_end()
    );
    let p = json_indent(&exemplo(Modelo::Proposta).unwrap(), 2);
    assert_eq!(
        p,
        include_str!("../fixtures/proposta.schema.json").trim_end()
    );
}

#[test]
fn exemplos_validam_e_voltam_iguais_apos_ida_e_volta_em_json() {
    let p = validar_proposta(&prop(json!({}))).unwrap();
    assert_eq!(
        validar_proposta(&serde_json::to_value(&p).unwrap()).unwrap(),
        p
    );
    let e = validar_explicacao(&expl(json!({}))).unwrap();
    assert_eq!(
        validar_explicacao(&serde_json::to_value(&e).unwrap()).unwrap(),
        e
    );
}

#[test]
fn dado_so_titulo_e_resumo_listas_vazias_opcionais_none_e_status_rascunho() {
    let base = json!({ "scope": "personal", "generatedAt": "2026-11-09T12:00:00Z", "title": "t", "summary": "s" });
    let mut p = base.clone();
    p["model"] = json!("proposta");
    p["schema"] = json!(1);
    p["kind"] = json!("correcao");
    let p = validar_proposta(&p).unwrap();
    assert_eq!(p.status, StatusProposta::Rascunho);
    assert_eq!(p.recommendation, None);
    assert_eq!(
        p.source,
        Fonte {
            repo: None,
            r#ref: None,
            paths: vec![]
        }
    );
    assert!(
        p.options.is_empty() && p.questions.is_empty() && p.plan.is_empty() && p.risks.is_empty()
    );

    let mut e = base;
    e["model"] = json!("explicacao");
    e["schema"] = json!(1);
    let e = validar_explicacao(&e).unwrap();
    assert_eq!(e.question, None);
    assert!(e.flow.is_empty() && e.sections.is_empty() && e.caveats.is_empty());
}

#[test]
fn null_vale_como_ausente_nos_opcionais_e_nos_padroes() {
    let p = validar_proposta(&prop(json!({
        "status": null, "source": null, "options": null, "recommendation": null,
        "questions": [{ "id": "Q1", "question": "q", "status": null }],
    })))
    .unwrap();
    assert_eq!(p.status, StatusProposta::Rascunho);
    assert_eq!(p.questions[0].status, StatusPergunta::Aberta);
    assert_eq!(p.questions[0].answer, None);
}

#[test]
fn recusa_com_o_caminho() {
    assert!(refused(validar_proposta(&prop(json!({ "kind": "outra" })))).starts_with("kind:"));
    let m = refused(validar_proposta(&prop(
        json!({ "recommendation": { "option": "Z", "why": [] } }),
    )));
    assert_eq!(m, "recommendation.option: não é o id de nenhuma opção");
    let mut opts = exemplo_proposta()["options"].as_array().unwrap().clone();
    opts.push(opts[0].clone());
    assert_eq!(
        refused(validar_proposta(&prop(json!({ "options": opts })))),
        "options[2].id: id repetido"
    );
    assert!(
        refused(validar_explicacao(&expl(
            json!({ "title": "x".repeat(501) })
        )))
        .starts_with("title:")
    );
    assert!(
        refused(validar_explicacao(&expl(json!({ "model": "proposta" })))).starts_with("model:")
    );
}

#[test]
fn tetos_proprios_do_documento() {
    let longo = |n: usize| json!("x".repeat(n));
    assert!(validar_explicacao(&expl(json!({ "summary": longo(4000) }))).is_ok());
    assert_eq!(
        refused(validar_explicacao(&expl(json!({ "summary": longo(4001) })))),
        "summary: texto acima de 4000 caracteres"
    );
    let trecho = |n: usize| json!([{ "title": "s", "code": [{ "file": "a.ts", "snippet": "x".repeat(n) }] }]);
    assert!(validar_explicacao(&expl(json!({ "sections": trecho(12000) }))).is_ok());
    assert_eq!(
        refused(validar_explicacao(&expl(
            json!({ "sections": trecho(12001) })
        ))),
        "sections[0].code[0].snippet: texto acima de 12000 caracteres"
    );
    let muitos: Vec<Value> = (0..41)
        .map(|_| json!({ "file": "a.ts", "snippet": "" }))
        .collect();
    assert_eq!(
        refused(validar_explicacao(&expl(
            json!({ "sections": [{ "title": "s", "code": muitos }] })
        ))),
        "sections[0].code: lista acima de 40 itens"
    );
    assert_eq!(
        refused(validar_explicacao(&expl(
            json!({ "glossary": [{ "term": "t", "meaning": 1 }] })
        ))),
        "glossary[0].meaning: esperava texto"
    );
}

#[test]
fn enums_e_ordem_das_recusas() {
    assert_eq!(
        refused(validar_proposta(&prop(
            json!({ "options": [{ "id": "A", "title": "t", "summary": "s", "effort": "enorme" }] })
        ))),
        "options[0].effort: esperava um de baixo, medio, alto"
    );
    assert_eq!(
        refused(validar_proposta(&prop(json!({ "status": "aprovada" })))),
        "status: esperava um de rascunho, em-discussao, decidida, descartada"
    );
    // options é conferido antes de kind, como no TS
    let m = refused(validar_proposta(&prop(
        json!({ "kind": "outra", "options": 1 }),
    )));
    assert_eq!(m, "options: esperava uma lista");
}

#[test]
fn proposta_serializa_com_as_chaves_na_ordem_do_ts() {
    let p = validar_proposta(&prop(json!({}))).unwrap();
    let v = serde_json::to_value(&p).unwrap();
    let chaves: Vec<_> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        chaves,
        [
            "model",
            "schema",
            "scope",
            "generatedAt",
            "kind",
            "status",
            "title",
            "summary",
            "source",
            "context",
            "problem",
            "goals",
            "nonGoals",
            "currentFlow",
            "proposedFlow",
            "options",
            "recommendation",
            "plan",
            "risks",
            "questions",
            "references",
        ]
    );
    assert_eq!(
        v["references"][0]["ref"],
        json!("observabilidade, consulta salva pedidos-presos")
    );
    assert_eq!(v["plan"][0]["where"], Value::Null);
}
