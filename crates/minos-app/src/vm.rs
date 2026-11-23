//! View-model do app: telas, linhas e textos, sem janela nem I/O.

use hades_ui::Tone;
use hades_ui::icons::IconName;
use minos_core::store::{ARTIFACT_HOST, Saidas, parse_artifact_url};
use minos_core::{Escopo, Modelo, Saida, modelos};

/// Telas do trilho (`design/minos.html`, plano F4 §6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tela {
    #[default]
    Publicacoes,
    NovaPagina,
    Time,
    Configuracoes,
}

impl Tela {
    pub const TODAS: [Tela; 4] = [
        Tela::Publicacoes,
        Tela::NovaPagina,
        Tela::Time,
        Tela::Configuracoes,
    ];

    pub fn rotulo(self) -> &'static str {
        match self {
            Tela::Publicacoes => "Publicações",
            Tela::NovaPagina => "Nova página",
            Tela::Time => "Página do time",
            Tela::Configuracoes => "Configurações",
        }
    }

    pub fn icone(self) -> IconName {
        match self {
            Tela::Publicacoes => IconName::Globe,
            Tela::NovaPagina => IconName::FilePen,
            Tela::Time => IconName::Users,
            Tela::Configuracoes => IconName::SlidersHorizontal,
        }
    }

    /// Nome de tela aceito em `--tela`.
    pub fn de_nome(nome: &str) -> Option<Tela> {
        match nome {
            "publicacoes" => Some(Tela::Publicacoes),
            "nova" => Some(Tela::NovaPagina),
            "time" => Some(Tela::Time),
            "configuracoes" => Some(Tela::Configuracoes),
            _ => None,
        }
    }

    pub fn de_rotulo(rotulo: &str) -> Option<Tela> {
        Tela::TODAS.into_iter().find(|t| t.rotulo() == rotulo)
    }
}

/// Atalho de teclado: `1` a `4` trocam de tela, na ordem do trilho; com um campo de texto focado a tecla é do campo.
pub fn tela_da_tecla(tecla: &str, campo_focado: bool) -> Option<Tela> {
    if campo_focado {
        return None;
    }
    let n: usize = tecla.parse().ok()?;
    Tela::TODAS.get(n.checked_sub(1)?).copied()
}

/// Controle de uma tarefa de fundo: no máximo uma em voo e resultado de geração velha é descartado (`invalidar`
/// quando a entrada mudou).
#[derive(Debug, Default)]
pub struct Tarefa {
    geracao: u64,
    em_voo: bool,
}

impl Tarefa {
    /// Geração da nova execução, ou `None` se a anterior ainda não terminou.
    pub fn iniciar(&mut self) -> Option<u64> {
        if self.em_voo {
            return None;
        }
        self.em_voo = true;
        Some(self.geracao)
    }

    /// Descarta o que estiver em voo e libera uma nova execução.
    pub fn invalidar(&mut self) {
        self.geracao = self.geracao.wrapping_add(1);
        self.em_voo = false;
    }

    /// `true` se o resultado da geração `geracao` ainda vale; só então libera a próxima execução.
    pub fn concluir(&mut self, geracao: u64) -> bool {
        if geracao != self.geracao {
            return false;
        }
        self.em_voo = false;
        true
    }

    pub fn carregando(&self) -> bool {
        self.em_voo
    }
}

/// Filtro por modelo da tela Publicações: "Todas" e um por modelo, na ordem do core.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filtro(Option<Modelo>);

impl Filtro {
    pub fn rotulos() -> Vec<&'static str> {
        std::iter::once("Todas")
            .chain(modelos().iter().map(|m| rotulo_do_modelo(*m)))
            .collect()
    }

    pub fn de_indice(i: usize) -> Filtro {
        Filtro(i.checked_sub(1).and_then(|i| modelos().get(i).copied()))
    }

    pub fn indice(self) -> usize {
        self.0
            .and_then(|m| modelos().iter().position(|x| *x == m))
            .map_or(0, |i| i + 1)
    }

    pub fn aceita(self, m: Modelo) -> bool {
        self.0.is_none_or(|f| f == m)
    }
}

/// Rótulo curto do modelo nas tabelas e no filtro.
pub fn rotulo_do_modelo(m: Modelo) -> &'static str {
    match m {
        Modelo::Orquestracao => "Orquestração",
        Modelo::Explicacao => "Explicação",
        Modelo::Proposta => "Proposta",
    }
}

/// Escopo como o `list` do CLI imprime (`commands/minos.ts:36`).
pub fn rotulo_do_escopo(e: Escopo) -> &'static str {
    if e == Escopo::Time { "time" } else { "pessoal" }
}

pub const SEM_LINK: &str = "sem link";

/// Contagem com o substantivo no número certo: "1 item", "3 itens".
pub fn itens(n: usize) -> String {
    if n == 1 {
        "1 item".to_owned()
    } else {
        format!("{n} itens")
    }
}

pub const COLUNAS_PUBLICACOES: [&str; 6] =
    ["Id", "Modelo", "Escopo", "Fonte", "Atualizado", "Link"];
pub const COLUNAS_TIME: [&str; 5] = ["Id", "Modelo", "Fonte", "Atualizado", "Link"];

/// Uma saída como linha de tabela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Linha {
    pub id: String,
    pub modelo: &'static str,
    pub escopo: &'static str,
    pub fonte: String,
    pub atualizado: String,
    pub link: String,
}

impl Linha {
    pub fn de(id: &str, s: &Saida) -> Linha {
        Linha {
            id: id.to_owned(),
            modelo: rotulo_do_modelo(s.model),
            escopo: rotulo_do_escopo(s.scope),
            fonte: nome_do_arquivo(&s.plan),
            atualizado: quando(&s.updated_at),
            link: s.url.clone().unwrap_or_else(|| SEM_LINK.to_owned()),
        }
    }

    pub fn celulas(&self) -> [String; 6] {
        [
            self.id.clone(),
            self.modelo.to_owned(),
            self.escopo.to_owned(),
            self.fonte.clone(),
            self.atualizado.clone(),
            self.link.clone(),
        ]
    }

    /// Células da Página do time: sem a coluna de escopo, que é sempre "time".
    pub fn celulas_do_time(&self) -> [String; 5] {
        [
            self.id.clone(),
            self.modelo.to_owned(),
            self.fonte.clone(),
            self.atualizado.clone(),
            self.link.clone(),
        ]
    }
}

/// Último componente do caminho da fonte; o caminho inteiro fica no detalhe.
pub fn nome_do_arquivo(caminho: &str) -> String {
    caminho
        .rsplit(['/', '\\'])
        .find(|p| !p.is_empty())
        .unwrap_or(caminho)
        .to_owned()
}

/// `AAAA-MM-DDTHH:MM:SS.sssZ` vira `AAAA-MM-DD HH:MM UTC`; o que não tiver esse formato passa como veio.
pub fn quando(iso: &str) -> String {
    let b = iso.as_bytes();
    let formato = b.len() >= 17 && b[4] == b'-' && b[7] == b'-' && b[10] == b'T' && b[13] == b':';
    if !formato || !iso.ends_with('Z') {
        return iso.to_owned();
    }
    format!("{} {} UTC", &iso[..10], &iso[11..16])
}

/// Linhas da tela Publicações, na ordem do registro.
pub fn linhas(saidas: &Saidas, filtro: Filtro) -> Vec<Linha> {
    saidas
        .iter()
        .filter(|(_, s)| filtro.aceita(s.model))
        .map(|(id, s)| Linha::de(id, s))
        .collect()
}

/// Linhas da Página do time: só as saídas `scope: team`.
pub fn linhas_do_time(saidas: &Saidas) -> Vec<Linha> {
    saidas
        .iter()
        .filter(|(_, s)| s.scope == Escopo::Time)
        .map(|(id, s)| Linha::de(id, s))
        .collect()
}

/// Posição da saída selecionada entre as linhas visíveis.
pub fn posicao_da_selecao(linhas: &[Linha], selecionada: Option<&str>) -> Option<usize> {
    let id = selecionada?;
    linhas.iter().position(|l| l.id == id)
}

/// Comando que a sessão roda depois de publicar; o espaço final espera a URL colada.
pub fn comando_do_link(id: &str) -> String {
    format!("hades minos link {id} ")
}

/// O que "Abrir" abre: o link registrado, ou a página local se não houver link (`commands/minos.ts:110`).
pub fn alvo_de_abrir(s: &Saida) -> String {
    s.url.clone().unwrap_or_else(|| s.file.clone())
}

/// Recusa da URL colada, com o texto do `link` do CLI (`commands/minos.ts:97`).
pub fn erro_de_url() -> String {
    format!("link: só https://{ARTIFACT_HOST}/artifact/<id> (ou /code/artifact/<id>)")
}

/// Recusa de saída sem registro, com o texto do CLI.
pub fn erro_de_saida(id: &str) -> String {
    format!("saída desconhecida: {id} (hades minos list)")
}

/// Confere a URL colada antes de gravar, como o `link` do CLI: campo vazio não é erro, só não grava.
pub fn conferir_link(bruto: &str) -> Result<Option<String>, String> {
    let bruto = bruto.trim();
    if bruto.is_empty() {
        return Ok(None);
    }
    parse_artifact_url(bruto).map(Some).ok_or_else(erro_de_url)
}

/// `teamEnabled` (`commands/minos.ts:25`): só `"1"` liga.
pub fn time_ligado(valor: &str) -> bool {
    valor == "1"
}

/// Estado de `HADES_MINOS_TEAM` na Página do time, com o aviso de exposição (`minos-publish.ts:64`, decisão 4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EstadoDoTime {
    pub rotulo: &'static str,
    pub tom: Tone,
    pub aviso: &'static str,
    pub dica: &'static str,
}

pub fn estado_do_time(ligado: bool) -> EstadoDoTime {
    if ligado {
        EstadoDoTime {
            rotulo: "ligada",
            tom: Tone::Warn,
            aviso: "A página do time está ligada (HADES_MINOS_TEAM=1): ela expõe o estado do trabalho a terceiros.",
            dica: "Publique com hades minos publish <modelo> <fonte> --team; desligue em Configurações → Minos.",
        }
    } else {
        EstadoDoTime {
            rotulo: "desligada",
            tom: Tone::Neutral,
            aviso: "A página do time está desligada (HADES_MINOS_TEAM=0): ela expõe o estado do trabalho a terceiros.",
            dica: "Ligue no onboarding (passo \"Página do time\") ou em Configurações → Minos.",
        }
    }
}

/// Texto do rodapé da tela Publicações.
pub fn rodape(total: usize, sem_link: usize) -> String {
    match total {
        0 => "nenhuma saída ainda".to_owned(),
        1 => format!("1 saída · {sem_link} sem link"),
        n => format!("{n} saídas · {sem_link} sem link"),
    }
}

#[cfg(test)]
mod tests;
