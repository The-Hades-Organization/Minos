//! Regra de negócio do Minos, sem interface.

pub mod check;
pub mod coleta;
pub mod documento;
pub mod modelo;
pub mod pagina;
pub mod store;

pub use check::{ContractError, Escopo};
pub use modelo::{Modelo, Validado, exemplo, modelos};
pub use store::{Saida, gravar_link, saidas};
