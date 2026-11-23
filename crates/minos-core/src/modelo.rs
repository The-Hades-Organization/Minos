//! Contrato dos modelos de saída (`src/lib/minos-model.ts`).
//!
//! Cada modelo declara um formato JSON validado na fronteira; a sessão gera os dados, nunca o HTML. `orquestracao` sai do
//! plano; `explicacao` e `proposta` ([`crate::documento`]) saem de um JSON que a sessão escreve. A ordem de avaliação
//! segue a do TS, para a primeira recusa ser a mesma.

use serde::Serialize;
use serde_json::Value;

use crate::check::{
    ContractError, Escopo, LIST_MAX, Resultado, SCOPES, TEXT_MAX, count, enum_texto, js_string,
    list, nullable_number, obj, one_of, text, utc,
};
use crate::documento::{self, Explicacao, Proposta};

/// `minos-model.ts:9`.
pub const ORQUESTRACAO_SCHEMA: u64 = 1;
const PCT_MAX: f64 = 100.0;

enum_texto!(
    /// Estado de um agente (`minos-model.ts:10`).
    EstadoAgente, AGENT_STATES {
        Working => "working", Idle => "idle", Blocked => "blocked", Unknown => "unknown", Done => "done",
        Archived => "archived",
    }
);

enum_texto!(
    /// Confiança de uma estimativa (`minos-model.ts:12`).
    Confianca, CONFIDENCES { Alta => "alta", Media => "media", Baixa => "baixa", Nenhuma => "nenhuma" }
);

/// `minos-model.ts:17`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unidade {
    pub name: String,
    pub role: String,
    pub depends_on: Vec<String>,
}

/// `minos-model.ts:18`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Agente {
    pub name: String,
    pub mode: String,
    pub state: EstadoAgente,
    pub phase: String,
    pub branch: String,
    pub pr: Option<u64>,
    pub open_questions: u64,
    pub report_done: bool,
}

/// `minos-model.ts:21`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pr {
    pub number: u64,
    pub branch: String,
    pub agent: String,
}

/// `minos-model.ts:22`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Escalacao {
    pub agent: String,
    pub question: String,
}

/// `minos-model.ts:23`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimativa {
    pub unit: String,
    pub cost_p50: Option<f64>,
    pub minutes_p50: Option<f64>,
    pub confidence: Confianca,
}

/// `minos-model.ts:24`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Limite {
    pub pct: f64,
    pub resets_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plano {
    pub title: String,
    pub objective: String,
    pub state: Vec<String>,
    pub units: Vec<Unidade>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Limites {
    pub five: Option<Limite>,
    pub seven: Option<Limite>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Consumo {
    pub estimate: Vec<Estimativa>,
    pub limits: Limites,
}

/// Dados do modelo `orquestracao` (`minos-model.ts:25`); serializa com as chaves na ordem do TS.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Orquestracao {
    pub model: Modelo,
    pub schema: u64,
    pub scope: Escopo,
    pub generated_at: String,
    pub plan: Plano,
    pub agents: Vec<Agente>,
    pub prs: Vec<Pr>,
    pub escalations: Vec<Escalacao>,
    pub consumption: Consumo,
}

/// Cabeçalho comum aos três modelos: `model` e `schema` (`minos-model.ts:47-48`, `minos-doc.ts:84-85`).
pub(crate) fn conferir_modelo(o: &crate::check::Obj, model: &str, schema: u64) -> Resultado<()> {
    if o.get("model").and_then(Value::as_str) != Some(model) {
        return Err(ContractError::new("model", format!("esperava \"{model}\"")));
    }
    if o.get("schema").and_then(Value::as_f64) != Some(schema as f64) {
        let v = js_string(o.get("schema"));
        return Err(ContractError::new(
            "schema",
            format!("versão {v} não suportada (esta é a {schema})"),
        ));
    }
    Ok(())
}

/// `scope` e `generatedAt`, nesta ordem (`minos-model.ts:55-56`, `minos-doc.ts:86`).
pub(crate) fn escopo(o: &crate::check::Obj) -> Resultado<(Escopo, String)> {
    let scope = one_of(o.get("scope"), SCOPES, "scope")?;
    let scope = Escopo::parse(scope).expect("valor da lista SCOPES");
    Ok((scope, utc(o.get("generatedAt"), "generatedAt")?))
}

/// `minos-model.ts:37`.
fn limite(value: Option<&Value>, path: &str) -> Resultado<Option<Limite>> {
    if let Some(Value::Null) = value {
        return Ok(None);
    }
    let o = obj(value, path)?;
    let at = format!("{path}.pct");
    let pct = match nullable_number(o.get("pct"), &at)? {
        Some(p) if p <= PCT_MAX => p,
        _ => return Err(ContractError::new(at, "esperava número de 0 a 100")),
    };
    Ok(Some(Limite {
        pct,
        resets_at: utc(o.get("resetsAt"), &format!("{path}.resetsAt"))?,
    }))
}

fn textos(value: Option<&Value>, path: &str) -> Resultado<Vec<String>> {
    list(value, path, LIST_MAX)?
        .iter()
        .enumerate()
        .map(|(i, s)| text(Some(s), &format!("{path}[{i}]"), TEXT_MAX))
        .collect()
}

fn mapear<T>(
    value: Option<&Value>,
    path: &str,
    f: impl Fn(&Value, String) -> Resultado<T>,
) -> Resultado<Vec<T>> {
    list(value, path, LIST_MAX)?
        .iter()
        .enumerate()
        .map(|(i, v)| f(v, format!("{path}[{i}]")))
        .collect()
}

fn campo(o: &crate::check::Obj, at: &str, k: &str) -> Resultado<String> {
    text(o.get(k), &format!("{at}.{k}"), TEXT_MAX)
}

/// `minos-model.ts:45`. Campo extra é ignorado (contrato aditivo).
pub fn validar_orquestracao(input: &Value) -> Resultado<Orquestracao> {
    let o = obj(Some(input), "$")?;
    conferir_modelo(o, "orquestracao", ORQUESTRACAO_SCHEMA)?;
    let plan = obj(o.get("plan"), "plan")?;
    let consumption = obj(o.get("consumption"), "consumption")?;
    let limits = obj(consumption.get("limits"), "consumption.limits")?;
    let (scope, generated_at) = escopo(o)?;
    let plan = Plano {
        title: text(plan.get("title"), "plan.title", TEXT_MAX)?,
        objective: text(plan.get("objective"), "plan.objective", TEXT_MAX)?,
        state: textos(plan.get("state"), "plan.state")?,
        units: mapear(plan.get("units"), "plan.units", |u, at| {
            let x = obj(Some(u), &at)?;
            Ok(Unidade {
                name: campo(x, &at, "name")?,
                role: campo(x, &at, "role")?,
                depends_on: textos(x.get("dependsOn"), &format!("{at}.dependsOn"))?,
            })
        })?,
    };
    let agents = mapear(o.get("agents"), "agents", |a, at| {
        let x = obj(Some(a), &at)?;
        let pr = match x.get("pr") {
            Some(Value::Null) => None,
            v => Some(count(v, &format!("{at}.pr"))?),
        };
        let Some(Value::Bool(report_done)) = x.get("reportDone") else {
            return Err(ContractError::new(
                format!("{at}.reportDone"),
                "esperava booleano",
            ));
        };
        Ok(Agente {
            name: campo(x, &at, "name")?,
            mode: campo(x, &at, "mode")?,
            state: one_of(x.get("state"), AGENT_STATES, &format!("{at}.state"))
                .map(|s| EstadoAgente::parse(s).unwrap())?,
            phase: campo(x, &at, "phase")?,
            branch: campo(x, &at, "branch")?,
            pr,
            open_questions: count(x.get("openQuestions"), &format!("{at}.openQuestions"))?,
            report_done: *report_done,
        })
    })?;
    let prs = mapear(o.get("prs"), "prs", |p, at| {
        let x = obj(Some(p), &at)?;
        Ok(Pr {
            number: count(x.get("number"), &format!("{at}.number"))?,
            branch: campo(x, &at, "branch")?,
            agent: campo(x, &at, "agent")?,
        })
    })?;
    let escalations = mapear(o.get("escalations"), "escalations", |e, at| {
        let x = obj(Some(e), &at)?;
        Ok(Escalacao {
            agent: campo(x, &at, "agent")?,
            question: campo(x, &at, "question")?,
        })
    })?;
    let estimate = mapear(
        consumption.get("estimate"),
        "consumption.estimate",
        |e, at| {
            let x = obj(Some(e), &at)?;
            Ok(Estimativa {
                unit: campo(x, &at, "unit")?,
                cost_p50: nullable_number(x.get("costP50"), &format!("{at}.costP50"))?,
                minutes_p50: nullable_number(x.get("minutesP50"), &format!("{at}.minutesP50"))?,
                confidence: one_of(
                    x.get("confidence"),
                    CONFIDENCES,
                    &format!("{at}.confidence"),
                )
                .map(|s| Confianca::parse(s).unwrap())?,
            })
        },
    )?;
    let limits = Limites {
        five: limite(limits.get("five"), "consumption.limits.five")?,
        seven: limite(limits.get("seven"), "consumption.limits.seven")?,
    };
    Ok(Orquestracao {
        model: Modelo::Orquestracao,
        schema: ORQUESTRACAO_SCHEMA,
        scope,
        generated_at,
        plan,
        agents,
        prs,
        escalations,
        consumption: Consumo { estimate, limits },
    })
}

/// De onde vêm os dados de um modelo (`minos-model.ts:102`): o plano da orquestração ou o JSON que a sessão escreve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origem {
    Plano,
    Json,
}

impl Origem {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plano => "plan",
            Self::Json => "json",
        }
    }
}

/// Modelos publicáveis (`minos-model.ts:105`), na ordem do `MODELS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modelo {
    Orquestracao,
    Explicacao,
    Proposta,
}

const MODELOS: &[Modelo] = &[Modelo::Orquestracao, Modelo::Explicacao, Modelo::Proposta];

/// Os modelos, na ordem do TS.
pub fn modelos() -> &'static [Modelo] {
    MODELOS
}

/// Exemplo que `schema <modelo>` imprime; `None` para quem sai do plano.
pub fn exemplo(m: Modelo) -> Option<Value> {
    match m {
        Modelo::Orquestracao => None,
        Modelo::Explicacao => Some(documento::exemplo_explicacao()),
        Modelo::Proposta => Some(documento::exemplo_proposta()),
    }
}

impl Modelo {
    pub fn id(self) -> &'static str {
        match self {
            Self::Orquestracao => "orquestracao",
            Self::Explicacao => "explicacao",
            Self::Proposta => "proposta",
        }
    }

    /// `isModelId` (`minos-model.ts:120`).
    pub fn from_id(id: &str) -> Option<Self> {
        MODELOS.iter().copied().find(|m| m.id() == id)
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Orquestracao => "Orquestração",
            Self::Explicacao => "Explicação de código",
            Self::Proposta => "Proposta",
        }
    }

    pub fn origem(self) -> Origem {
        match self {
            Self::Orquestracao => Origem::Plano,
            Self::Explicacao | Self::Proposta => Origem::Json,
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Self::Orquestracao => {
                "acompanhamento da orquestração: plano, agentes, PRs, escalações e consumo"
            }
            Self::Explicacao => {
                "explicação de código: resumo, fluxo passo a passo com arquivo:linha, trechos, arquivos, glossário e ressalvas"
            }
            Self::Proposta => {
                "proposta ou discussão de implementação, correção, fluxo ou refatoração: problema, opções comparadas, recomendação, plano, riscos e perguntas"
            }
        }
    }

    /// O `validate` do modelo.
    pub fn validar(self, input: &Value) -> Resultado<Validado> {
        Ok(match self {
            Self::Orquestracao => Validado::Orquestracao(validar_orquestracao(input)?),
            Self::Explicacao => Validado::Explicacao(documento::validar_explicacao(input)?),
            Self::Proposta => Validado::Proposta(documento::validar_proposta(input)?),
        })
    }
}

impl Serialize for Modelo {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.id())
    }
}

/// Dados de qualquer modelo depois da fronteira.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Validado {
    Orquestracao(Orquestracao),
    Explicacao(Explicacao),
    Proposta(Proposta),
}

impl Validado {
    pub fn modelo(&self) -> Modelo {
        match self {
            Self::Orquestracao(_) => Modelo::Orquestracao,
            Self::Explicacao(_) => Modelo::Explicacao,
            Self::Proposta(_) => Modelo::Proposta,
        }
    }

    /// O JSON dos dados, com as chaves na ordem do TS.
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("dados validados sempre serializam")
    }
}

#[cfg(test)]
#[path = "modelo_tests.rs"]
mod tests;
