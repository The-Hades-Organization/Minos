//! Regra de negócio do Minos, sem interface.

pub mod check;
pub mod coleta;
pub mod documento;
pub mod guarda;
pub mod modelo;
pub mod pagina;
pub mod pagina_doc;
pub mod publicar;
pub mod store;
pub mod varredura;

pub use check::{ContractError, Escopo};
pub use modelo::{Modelo, Validado, exemplo, modelos};
pub use store::{Saida, gravar_link, saidas};
