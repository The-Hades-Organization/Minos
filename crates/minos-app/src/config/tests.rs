use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use super::*;

const KIT: &str = "\
HADES_MINOS_TEAM\t0\tbool\tMinos\tPágina do time\tnext\tPublica a página do time.\n\
HADES_OUTRA\t1\tbool\tG\tOutra\tnext\tNão é do Minos.\n";

struct Sandbox {
    _dir: TempDir,
    env: Env,
    cwd: PathBuf,
    dados: PathBuf,
}

fn sandbox() -> Sandbox {
    let dir = TempDir::new().unwrap();
    let kit = dir.path().join("skill/kit");
    let dados = dir.path().join("dados");
    fs::create_dir_all(&kit).unwrap();
    fs::create_dir_all(&dados).unwrap();
    fs::write(kit.join("config.tsv"), KIT).unwrap();
    let env = Env::empty()
        .with("HOME", dir.path().to_string_lossy())
        .with(
            "HADES_SKILL_DIR",
            dir.path().join("skill").to_string_lossy(),
        )
        .with("HADES_DATA", dados.to_string_lossy());
    Sandbox {
        cwd: dir.path().to_path_buf(),
        env,
        dados,
        _dir: dir,
    }
}

fn valor(s: &Sandbox) -> String {
    config::config_value(&s.env, &s.cwd, CHAVE, "?")
}

#[test]
fn le_a_chave_do_catalogo_no_padrao() {
    let s = sandbox();
    let linha = ConfigLocal::new(&s.env, &s.cwd).carregar().unwrap();
    assert_eq!(linha.rotulo, "Página do time");
    assert_eq!(linha.descricao, "Publica a página do time.");
    assert_eq!(linha.padrao, "0");
    assert_eq!(linha.valor, "0");
    assert_eq!(linha.origem, SettingOrigin::Default);
    assert!(!linha.ligado());
}

#[test]
fn ligar_grava_no_config_env_e_desligar_volta_ao_padrao() {
    let s = sandbox();
    let fonte = ConfigLocal::new(&s.env, &s.cwd);
    assert!(fonte.gravavel());
    let linha = fonte.carregar().unwrap();
    assert!(alternar(&fonte, &linha, true).unwrap());
    assert_eq!(valor(&s), "1");
    let linha = fonte.carregar().unwrap();
    assert_eq!(linha.origem, SettingOrigin::File);
    assert!(!alternar(&fonte, &linha, false).unwrap());
    assert_eq!(valor(&s), "0");
    let texto = fs::read_to_string(s.dados.join("config.env")).unwrap_or_default();
    assert!(!texto.contains(CHAVE), "{texto}");
    assert_eq!(fonte.carregar().unwrap().origem, SettingOrigin::Default);
}

#[test]
fn exportada_no_ambiente_e_so_leitura() {
    let s = sandbox();
    let env = s.env.clone().with(CHAVE, "1");
    let fonte = ConfigLocal::new(&env, &s.cwd);
    let linha = fonte.carregar().unwrap();
    assert_eq!(linha.origem, SettingOrigin::Env);
    assert!(linha.ligado());
    assert!(
        alternar(&fonte, &linha, false)
            .unwrap_err()
            .contains("ambiente")
    );
    assert!(!s.dados.join("config.env").exists());
}

#[test]
fn sem_instalacao_nao_grava() {
    let dir = TempDir::new().unwrap();
    let kit = dir.path().join("skill/kit");
    fs::create_dir_all(&kit).unwrap();
    fs::write(kit.join("config.tsv"), KIT).unwrap();
    let env = Env::empty()
        .with("HOME", dir.path().to_string_lossy())
        .with(
            "HADES_SKILL_DIR",
            dir.path().join("skill").to_string_lossy(),
        );
    let fonte = ConfigLocal::new(&env, dir.path());
    assert!(!fonte.gravavel());
    let linha = fonte.carregar().unwrap();
    assert!(
        alternar(&fonte, &linha, true)
            .unwrap_err()
            .contains("sem instalação")
    );
    assert!(!kit.join("config.env").exists());
}
