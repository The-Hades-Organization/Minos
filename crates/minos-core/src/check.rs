//! Conferências de fronteira que todos os modelos usam (`src/lib/minos-check.ts`).
//!
//! Tipo errado, enum desconhecido, texto ou lista acima do teto são recusados com o caminho do campo, nunca coagidos; o
//! valor não entra na mensagem (ele pode ser segredo). Campo ausente chega como `None` (o `undefined` do TS), distinto de
//! `Some(Value::Null)`.

use std::fmt;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

/// Teto padrão de texto, em unidades UTF-16 como o `length` do JS (`minos-check.ts:7`).
pub const TEXT_MAX: usize = 500;
/// Teto padrão de lista (`minos-check.ts:8`).
pub const LIST_MAX: usize = 200;

/// `minos-check.ts:9`, com `[0-9]` no lugar do `\d` (no `regex` do Rust, `\d` casa dígito Unicode).
static ISO_UTC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^([0-9]{4})-([0-9]{2})-([0-9]{2})T([0-9]{2}):([0-9]{2}):([0-9]{2})(?:\.([0-9]+))?Z$",
    )
    .unwrap()
});

/// Enum de texto fechado: variantes, lista dos valores na ordem do TS, `as_str`, `parse` e `Serialize` como texto.
macro_rules! enum_texto {
    ($(#[$m:meta])* $nome:ident, $todos:ident { $($var:ident => $txt:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $nome { $($var),+ }

        /// Valores aceitos, na ordem do TS.
        pub const $todos: &[&str] = &[$($txt),+];

        impl $nome {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$var => $txt),+ }
            }

            pub fn parse(s: &str) -> Option<Self> {
                match s { $($txt => Some(Self::$var),)+ _ => None }
            }
        }

        impl serde::Serialize for $nome {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }
    };
}
pub(crate) use enum_texto;

enum_texto!(
    /// Escopo de uma saída (`minos-check.ts:4`).
    Escopo, SCOPES { Pessoal => "personal", Time => "team" }
);

/// Recusa na fronteira: `"<caminho>: <motivo>"` (`minos-check.ts:11`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractError {
    pub path: String,
    pub reason: String,
}

impl ContractError {
    pub fn new(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for ContractError {}

pub type Resultado<T> = Result<T, ContractError>;
pub type Obj = Map<String, Value>;

/// Comprimento como o `String.length` do JS: unidades UTF-16.
pub fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `minos-check.ts:23`.
pub fn obj<'a>(value: Option<&'a Value>, path: &str) -> Resultado<&'a Obj> {
    match value {
        Some(Value::Object(o)) => Ok(o),
        _ => Err(ContractError::new(path, "esperava um objeto")),
    }
}

/// `minos-check.ts:28`.
pub fn text(value: Option<&Value>, path: &str, max: usize) -> Resultado<String> {
    let Some(Value::String(s)) = value else {
        return Err(ContractError::new(path, "esperava texto"));
    };
    if js_len(s) > max {
        return Err(ContractError::new(
            path,
            format!("texto acima de {max} caracteres"),
        ));
    }
    Ok(s.clone())
}

/// `minos-check.ts:34`.
pub fn list<'a>(value: Option<&'a Value>, path: &str, max: usize) -> Resultado<&'a [Value]> {
    let Some(Value::Array(a)) = value else {
        return Err(ContractError::new(path, "esperava uma lista"));
    };
    if a.len() > max {
        return Err(ContractError::new(
            path,
            format!("lista acima de {max} itens"),
        ));
    }
    Ok(a)
}

/// `minos-check.ts:40`. Inteiro acima de `u64::MAX` (aceito pelo JS) é recusado aqui.
pub fn count(value: Option<&Value>, path: &str) -> Resultado<u64> {
    let erro = || ContractError::new(path, "esperava inteiro não negativo");
    let Some(Value::Number(n)) = value else {
        return Err(erro());
    };
    if let Some(u) = n.as_u64() {
        return Ok(u);
    }
    match n.as_f64() {
        Some(f) if f.fract() == 0.0 && f >= 0.0 && f <= u64::MAX as f64 => Ok(f as u64),
        _ => Err(erro()),
    }
}

/// `minos-check.ts:45`. `null` vira `None`; ausente é recusado.
pub fn nullable_number(value: Option<&Value>, path: &str) -> Resultado<Option<f64>> {
    match value {
        Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => match n.as_f64() {
            Some(f) if f.is_finite() && f >= 0.0 => Ok(Some(f)),
            _ => Err(nn_erro(path)),
        },
        _ => Err(nn_erro(path)),
    }
}

fn nn_erro(path: &str) -> ContractError {
    ContractError::new(path, "esperava número finito não negativo ou null")
}

/// `minos-check.ts:51`.
pub fn one_of<'a>(value: Option<&'a Value>, allowed: &[&str], path: &str) -> Resultado<&'a str> {
    match value {
        Some(Value::String(s)) if allowed.contains(&s.as_str()) => Ok(s),
        _ => Err(ContractError::new(
            path,
            format!("esperava um de {}", allowed.join(", ")),
        )),
    }
}

/// `minos-check.ts:56`. Além do formato, o instante tem de passar no `Date.parse` do Bun (JavaScriptCore): mês 1–12,
/// dia 1–31 sem conferir o mês, hora 0–24 (24 só com o resto zerado), minuto e segundo 0–59.
pub fn utc(value: Option<&Value>, path: &str) -> Resultado<String> {
    let t = text(value, path, TEXT_MAX)?;
    if !instante_valido(&t) {
        return Err(ContractError::new(
            path,
            "esperava instante ISO 8601 em UTC (com Z)",
        ));
    }
    Ok(t)
}

fn instante_valido(t: &str) -> bool {
    let Some(c) = ISO_UTC.captures(t) else {
        return false;
    };
    let n = |i: usize| c[i].parse::<u32>().unwrap_or(u32::MAX);
    let (mes, dia, hora, min, seg) = (n(2), n(3), n(4), n(5), n(6));
    let fracao_zero = c
        .get(7)
        .is_none_or(|f| f.as_str().bytes().all(|b| b == b'0'));
    let hora_ok = hora < 24 || (hora == 24 && min == 0 && seg == 0 && fracao_zero);
    (1..=12).contains(&mes) && (1..=31).contains(&dia) && hora_ok && min < 60 && seg < 60
}

/// `String(value)` do JS, para as mensagens que o TS monta com ele.
pub fn js_string(value: Option<&Value>) -> String {
    match value {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(v @ Value::Number(_)) => hades_core::jsonfmt::json_compact(v),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    js_string(Some(v))
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn msg<T: fmt::Debug>(r: Resultado<T>) -> String {
        r.unwrap_err().to_string()
    }

    #[test]
    fn obj_recusa_lista_null_e_ausente() {
        assert!(obj(Some(&json!({})), "$").is_ok());
        assert_eq!(
            msg(obj(Some(&json!([])), "plan")),
            "plan: esperava um objeto"
        );
        assert_eq!(msg(obj(Some(&Value::Null), "$")), "$: esperava um objeto");
        assert_eq!(msg(obj(None, "$")), "$: esperava um objeto");
    }

    #[test]
    fn text_conta_utf16_e_nao_repete_o_valor() {
        assert!(text(Some(&json!("x".repeat(500))), "t", TEXT_MAX).is_ok());
        assert_eq!(
            msg(text(Some(&json!("x".repeat(501))), "t", TEXT_MAX)),
            "t: texto acima de 500 caracteres"
        );
        // 😀 ocupa duas unidades UTF-16, como no JS
        assert!(text(Some(&json!("😀")), "t", 1).is_err());
        assert!(text(Some(&json!("é")), "t", 1).is_ok());
        assert_eq!(
            msg(text(Some(&json!(12)), "t", TEXT_MAX)),
            "t: esperava texto"
        );
    }

    #[test]
    fn list_e_count() {
        assert_eq!(
            msg(list(Some(&json!([1, 2, 3])), "l", 2)),
            "l: lista acima de 2 itens"
        );
        assert_eq!(msg(list(Some(&json!({})), "l", 2)), "l: esperava uma lista");
        assert_eq!(count(Some(&json!(3.0)), "c"), Ok(3));
        assert_eq!(count(Some(&json!(-0.0)), "c"), Ok(0));
        for v in [json!(1.5), json!(-1), json!("doze"), Value::Null] {
            assert_eq!(
                msg(count(Some(&v), "c")),
                "c: esperava inteiro não negativo"
            );
        }
        assert!(count(None, "c").is_err());
    }

    #[test]
    fn nullable_number_distingue_null_de_ausente() {
        assert_eq!(nullable_number(Some(&Value::Null), "n"), Ok(None));
        assert_eq!(nullable_number(Some(&json!(2.5)), "n"), Ok(Some(2.5)));
        assert!(nullable_number(None, "n").is_err());
        assert!(nullable_number(Some(&json!(-1)), "n").is_err());
    }

    #[test]
    fn one_of_lista_os_aceitos() {
        assert_eq!(one_of(Some(&json!("team")), SCOPES, "scope"), Ok("team"));
        assert_eq!(
            msg(one_of(Some(&json!("publico")), SCOPES, "scope")),
            "scope: esperava um de personal, team"
        );
    }

    #[test]
    fn utc_segue_o_date_parse_do_bun() {
        for ok in [
            "2026-11-04T12:00:00Z",
            "2026-02-31T00:00:00Z",
            "2026-01-01T24:00:00.000Z",
            "0000-01-01T00:00:00Z",
            "2026-01-01T00:00:00.1234567Z",
        ] {
            assert!(utc(Some(&json!(ok)), "g").is_ok(), "{ok}");
        }
        for ruim in [
            "2026-11-04T12:00:00-03:00",
            "2026-02-32T00:00:00Z",
            "2026-13-01T00:00:00Z",
            "2026-00-01T00:00:00Z",
            "2026-01-00T00:00:00Z",
            "2026-01-01T24:00:01Z",
            "2026-01-01T24:00:00.0004Z",
            "2026-01-01T23:60:00Z",
            "2026-01-01T23:59:60Z",
            "2026-01-01T00:00:00.Z",
            "2026-01-01T00:00:00Z\n",
            "٢٠٢٦-01-01T00:00:00Z",
        ] {
            assert_eq!(
                msg(utc(Some(&json!(ruim)), "g")),
                "g: esperava instante ISO 8601 em UTC (com Z)",
                "{ruim}"
            );
        }
    }

    #[test]
    fn js_string_como_o_string_do_js() {
        assert_eq!(js_string(None), "undefined");
        assert_eq!(js_string(Some(&json!(2.0))), "2");
        assert_eq!(js_string(Some(&json!([1, null, "a"]))), "1,,a");
        assert_eq!(js_string(Some(&json!({"a": 1}))), "[object Object]");
    }
}
