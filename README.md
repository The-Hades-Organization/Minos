# Minos

Minos: ferramenta do Hades, reescrita em Rust.

## Crates

- `crates/minos-core`: regra de negócio, sem interface.
- `crates/minos-cli`: binário `minos` (clap).
- `crates/minos-app`: view gpui exportada e binário `minos-app`.

## Estado

F0, em construção. Só o esqueleto do workspace existe; nada da ferramenta foi portado ainda.

## Capturas

O `minos-app` nos temas claro e escuro. Os dados são fictícios, montados num HOME de exemplo.

| Tela | Claro | Escuro |
| --- | --- | --- |
| Publicações, com o detalhe de uma saída aberto | ![Publicações, tema claro](docs/identidade/minos-publicacoes-claro.png) | ![Publicações, tema escuro](docs/identidade/minos-publicacoes-escuro.png) |
| Nova página, com a prévia de uma proposta | ![Nova página, tema claro](docs/identidade/minos-nova-claro.png) | ![Nova página, tema escuro](docs/identidade/minos-nova-escuro.png) |
| Nova página, com a fonte recusada pelo modelo | ![Nova página com recusa, tema claro](docs/identidade/minos-nova-recusa-claro.png) | ![Nova página com recusa, tema escuro](docs/identidade/minos-nova-recusa-escuro.png) |
| Página do time | ![Página do time, tema claro](docs/identidade/minos-time-claro.png) | ![Página do time, tema escuro](docs/identidade/minos-time-escuro.png) |
| Configurações | ![Configurações, tema claro](docs/identidade/minos-configuracoes-claro.png) | ![Configurações, tema escuro](docs/identidade/minos-configuracoes-escuro.png) |

## Relação com o Hades-Skill

Faz parte da reescrita em Rust das ferramentas do [Hades-Skill](https://github.com/The-Hades-Organization/Hades-Skill). O Hades-Skill continua sendo a fonte até o corte, quando passa a chamar o binário.

## Licença

MIT. Veja `LICENSE`.
