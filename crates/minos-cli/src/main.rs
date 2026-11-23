//! `minos`: o `hades minos` (`src/commands/minos.ts`) em Rust. Parser à mão como o TS, mesmos textos e códigos de saída.
//! Publicar num Artifact é ação da sessão do Claude, não da CLI: o `publish` grava a página e devolve o que publicar.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use hades_core::clock::now_ms;
use hades_core::config::config_value;
use hades_core::env::Env;
use hades_core::errors::{render_cli, render_error};
use hades_core::fsx::{read_text, resolve};
use hades_core::jsonfmt::json_indent;
use hades_core::output::{HadesError, out, warn};
use hades_core::paths::{find_state_dir, skill_root};
use hades_core::proc::{RunOptions, run, which};
use hades_core::work::{WorkOptions, load_work_env};
use minos_core::Escopo;
use minos_core::coleta::{ErroColeta, collect};
use minos_core::modelo::{Modelo, Origem, Orquestracao, exemplo, modelos};
use minos_core::publicar::{Pedido, Publicado, TipoRecusa};
use minos_core::store::{
    ARTIFACT_HOST, ErroLink, gravar_link, is_output_id, parse_artifact_url, saidas,
};

const OK: u8 = 0;
const FAIL: u8 = 1;
const USAGE_RC: u8 = 2;
/// `OPEN_TIMEOUT_MS` (`commands/minos.ts:24`).
const OPEN_TIMEOUT: Duration = Duration::from_millis(5000);
const PREFIX: &str = "hades";
const UNEXPECTED: &str = "cli_unexpected";

/// `USAGE` (`commands/minos.ts:14`).
const USAGE: &str = "hades minos list | schema <modelo> | publish <modelo> <fonte> [--html <arquivo>] [--team] | link <saída> <url> | open <saída> [--print]
  list      os modelos e as saídas já preparadas, com o link registrado de cada uma
  schema    o esqueleto JSON de um modelo de documento (explicacao, proposta), para a sessão preencher
  publish   monta a página do modelo, passa o Cerberus e grava; devolve o arquivo que a sessão publica com a ferramenta
            Artifact (e o link já registrado, para atualizar no mesmo lugar). <fonte> é o plano.md (orquestracao) ou o
            JSON de dados (explicacao, proposta). --html grava só o arquivo offline.
            --team é a página do time: recusa enquanto HADES_MINOS_TEAM=0
  link      grava o link devolvido pela ferramenta Artifact (https://claude.ai/…/artifact/<id>)
  open      abre o link registrado, ou a página local se não houver link (--print só mostra o destino)";

/// Fim antecipado do comando: o código de saída, com o stderr já escrito.
type Saida = Result<u8, u8>;

/// `usage` (`commands/minos.ts:26`).
fn usage(message: &str) -> u8 {
    warn(&format!("{message}\n{USAGE}"));
    USAGE_RC
}

fn unexpected() -> u8 {
    warn(&render_cli("erro inesperado", Some(UNEXPECTED), PREFIX));
    FAIL
}

/// `stateDir` (`src/lib/kit.ts:27`): sem instalação, o `die` com `EXIT.fail`.
fn state_dir(env: &Env) -> Result<PathBuf, u8> {
    let cwd = std::env::current_dir().unwrap_or_default();
    find_state_dir(env, &cwd).ok_or_else(|| {
        let e = HadesError::with_code(
            "sem instalação do Hades para guardar o estado: rode kit/install.sh (ou defina HADES_DATA)",
            i32::from(FAIL),
        );
        warn(&render_error(&e, PREFIX));
        FAIL
    })
}

fn model_ids() -> String {
    modelos()
        .iter()
        .map(|m| m.id())
        .collect::<Vec<_>>()
        .join(", ")
}

fn unknown_model(id: &str) -> u8 {
    usage(&format!("modelo desconhecido: {id} ({})", model_ids()))
}

/// `list` (`commands/minos.ts:31`).
fn list(env: &Env) -> Saida {
    out("modelos:");
    for m in modelos() {
        let source = match m.origem() {
            Origem::Plano => "<plano.md>",
            Origem::Json => "<dados.json>",
        };
        out(&format!("  {}  {source}  {}", m.id(), m.describe()));
    }
    let outputs = saidas(&state_dir(env)?);
    out(if outputs.is_empty() {
        "saídas: nenhuma ainda (hades minos publish orquestracao <plano.md>)"
    } else {
        "saídas:"
    });
    for (id, o) in outputs {
        let scope = if o.scope == Escopo::Time {
            "time"
        } else {
            "pessoal"
        };
        let url = o
            .url
            .as_deref()
            .unwrap_or("sem link (publique e rode hades minos link)");
        out(&format!("  {id}  {scope}  {url}  {}", o.updated_at));
    }
    Ok(OK)
}

/// `schema` (`commands/minos.ts:40`).
fn schema(args: &[String]) -> Saida {
    let [id] = args else {
        return Err(usage("schema pede <modelo>"));
    };
    let model = Modelo::from_id(id).ok_or_else(|| unknown_model(id))?;
    let Some(example) = exemplo(model) else {
        return Err(usage(&format!(
            "{id} não sai de JSON: a fonte é o plano.md (hades minos publish {id} <plano.md>)"
        )));
    };
    out(&json_indent(&example, 2));
    Ok(OK)
}

/// `PublishArgs` (`commands/minos.ts:50`).
struct ArgsPublish {
    model: String,
    plan: String,
    html: Option<String>,
    team: bool,
}

/// `parsePublish` (`commands/minos.ts:52`).
fn parse_publish(args: &[String]) -> Result<ArgsPublish, String> {
    let mut positional = Vec::new();
    let mut html = None;
    let mut team = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--team" => team = true,
            "--html" => html = Some(it.next().ok_or("--html pede um arquivo")?.clone()),
            a if a.starts_with("--") => return Err(format!("opção desconhecida: {a}")),
            a => positional.push(a),
        }
    }
    match positional.as_slice() {
        [model, plan] => Ok(ArgsPublish {
            model: (*model).to_owned(),
            plan: (*plan).to_owned(),
            html,
            team,
        }),
        _ => Err("publish pede <modelo> e <fonte> (plano.md ou dados.json)".to_owned()),
    }
}

/// O `collectOrchestration` padrão da CLI: lê o plano antes do registro, e o `loadWorkEnv` que falha vira recusa
/// `input`, como o erro que o `publish` do TS captura.
fn collect_cli(
    env: &Env,
    cwd: &Path,
    plan: &Path,
    scope: Escopo,
    now_ms: i64,
) -> Result<Orquestracao, ErroColeta> {
    if read_text(plan).is_none() {
        return Err(ErroColeta::PlanoAusente(plan.to_owned()));
    }
    let work =
        load_work_env(&WorkOptions::default(), env).map_err(|e| ErroColeta::Ambiente(e.message))?;
    collect(plan, scope, now_ms, &work, env, cwd)
}

/// `publishCommand` (`commands/minos.ts:72`).
fn publish(env: &Env, args: &[String]) -> Saida {
    let parsed = parse_publish(args).map_err(|m| usage(&m))?;
    let model = Modelo::from_id(&parsed.model).ok_or_else(|| unknown_model(&parsed.model))?;
    let state = state_dir(env)?;
    let cwd = std::env::current_dir().unwrap_or_default();
    let now_ms = now_ms(env).map_err(|_| unexpected())?;
    let coletor = |plan: &Path, scope: Escopo, now: i64| collect_cli(env, &cwd, plan, scope, now);
    let req = Pedido {
        model,
        plan: resolve(Path::new(&parsed.plan))
            .to_string_lossy()
            .into_owned(),
        scope: if parsed.team {
            Escopo::Time
        } else {
            Escopo::Pessoal
        },
        html_file: parsed.html,
        state: state.clone(),
        team_enabled: config_value(env, &cwd, "HADES_MINOS_TEAM", "0") == "1",
        credentials: read_text(&state.join("credenciais.md")),
        skill_roots: vec![skill_root(env)],
        cwd: cwd.clone(),
        now_ms,
        collect: &coletor,
    };
    match minos_core::publicar::publish(&req, env) {
        Err(r) => {
            let head = if r.kind == TipoRecusa::Guard {
                "o Cerberus barrou a publicação (nada foi gerado). Limpe e rode de novo:"
            } else {
                "minos: recusado:"
            };
            let lines: Vec<String> = r.reasons.iter().map(|m| format!("  {m}")).collect();
            out(&format!("{head}\n{}", lines.join("\n")));
            Err(if r.kind == TipoRecusa::Input {
                USAGE_RC
            } else {
                FAIL
            })
        }
        Ok(Publicado::Offline { file }) => {
            out(&format!(
                "arquivo: {}\nPágina autocontida (sem script nem recurso externo); nada saiu da máquina.",
                file.display()
            ));
            Ok(OK)
        }
        Ok(Publicado::Artifact {
            id,
            file,
            url,
            description,
            ..
        }) => {
            out(&[
                format!("publique: {}", file.display()),
                format!(
                    "link registrado: {}",
                    url.as_deref().unwrap_or("nenhum (primeira publicação)")
                ),
                format!("descrição: {description}"),
                format!("depois: hades minos link {id} <url devolvida pela ferramenta Artifact>"),
            ]
            .join("\n"));
            Ok(OK)
        }
    }
}

/// `link` (`commands/minos.ts:97`).
fn link(env: &Env, args: &[String]) -> Saida {
    let [id, raw] = args else {
        return Err(usage("link pede <saída> e <url>"));
    };
    let url_err = || {
        usage(&format!(
            "link: só https://{ARTIFACT_HOST}/artifact/<id> (ou /code/artifact/<id>)"
        ))
    };
    parse_artifact_url(raw).ok_or_else(url_err)?;
    let state = state_dir(env)?;
    let url = gravar_link(&state, id, raw).map_err(|e| match e {
        ErroLink::Url => url_err(),
        ErroLink::SaidaDesconhecida => {
            usage(&format!("saída desconhecida: {id} (hades minos list)"))
        }
        ErroLink::Io(_) => unexpected(),
    })?;
    out(&format!("link de {id} gravado: {url}"));
    Ok(OK)
}

/// `open` (`commands/minos.ts:110`).
fn open(env: &Env, args: &[String]) -> Saida {
    let print_only = args.iter().any(|a| a == "--print");
    let rest: Vec<&String> = args.iter().filter(|a| *a != "--print").collect();
    let [id] = rest.as_slice() else {
        return Err(usage("open pede <saída>"));
    };
    let current = if is_output_id(id) {
        saidas(&state_dir(env)?).into_iter().find(|(k, _)| k == *id)
    } else {
        None
    };
    let Some((_, current)) = current else {
        return Err(usage(&format!(
            "saída desconhecida: {id} (hades minos list)"
        )));
    };
    let target = current.url.unwrap_or(current.file);
    out(&target);
    let path = env.get_raw("PATH").unwrap_or("");
    let opener = ["xdg-open", "open"]
        .into_iter()
        .find(|c| which(c, path).is_some());
    if let (false, Some(opener)) = (print_only, opener) {
        let options = RunOptions {
            timeout: Some(OPEN_TIMEOUT),
            ..RunOptions::default()
        };
        run(opener, &[&target], &options);
    }
    Ok(OK)
}

/// `minosCommand` (`commands/minos.ts:122`).
fn minos(env: &Env, args: &[String]) -> u8 {
    let Some((sub, rest)) = args.split_first() else {
        return usage("falta o subcomando");
    };
    let result = match sub.as_str() {
        "list" => list(env),
        "schema" => schema(rest),
        "publish" => publish(env, rest),
        "link" => link(env, rest),
        "open" => open(env, rest),
        other => Err(usage(&format!("subcomando desconhecido: {other}"))),
    };
    result.unwrap_or_else(|rc| rc)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    ExitCode::from(minos(&Env::from_process(), &args))
}
