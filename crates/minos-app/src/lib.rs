//! View gpui do Minos, exportada para o app próprio e para o painel do Hades.

mod config;
mod nova;
mod service;
mod view;
pub mod vm;

pub use view::{Abertura, MinosView, abrir_janela, abrir_janela_com, abrir_janela_em};

pub const NOME: &str = "Minos";
