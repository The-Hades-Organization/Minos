//! Regra de negócio do Minos, sem interface.

pub mod check;
pub mod documento;
pub mod modelo;

pub use check::{ContractError, Escopo};
pub use modelo::{Modelo, Validado, exemplo, modelos};
