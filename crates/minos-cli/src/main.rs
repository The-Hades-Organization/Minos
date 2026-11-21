use clap::Parser;

/// Minos: ferramenta do Hades, reescrita em Rust.
#[derive(Parser)]
#[command(name = "minos", version, about)]
struct Cli {}

fn main() {
    Cli::parse();
}
