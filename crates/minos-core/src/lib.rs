//! Regra de negócio do Minos, sem interface.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compila() {
        assert_eq!(env!("CARGO_PKG_NAME"), "minos-core");
    }
}
