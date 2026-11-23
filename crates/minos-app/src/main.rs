use hades_ui::ThemeMode;
use minos_app::Abertura;
use minos_app::vm::Tela;
use minos_core::Modelo;

fn valor_de(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let modo = if std::env::args().any(|a| a == "--dark") {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    let abertura = Abertura {
        tela: valor_de("--tela")
            .and_then(|n| Tela::de_nome(&n))
            .unwrap_or_default(),
        saida: valor_de("--saida"),
        modelo: valor_de("--modelo").and_then(|m| Modelo::from_id(&m)),
        fonte: valor_de("--fonte").map(Into::into),
    };
    gpui_platform::application().run(move |cx| {
        hades_ui::init(cx, modo);
        minos_app::abrir_janela_com(cx, abertura);
    });
}
