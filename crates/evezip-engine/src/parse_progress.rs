/// Extrai percentual de tokens de progresso do 7zz com -bsp1.
/// O 7zz escreve no stdout tokens como "  4% 12 - nome/do/arquivo",
/// separados por '\r' (e '\n' no fim). Retorna o número antes de '%'.
pub fn parse_progress(token: &str) -> Option<u8> {
    let t = token.trim_start();
    let pos = t.find('%')?;
    let digitos = &t[..pos];
    if digitos.is_empty() || !digitos.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digitos.parse::<u16>().ok().map(|v| v.min(100) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrai_percentual() {
        assert_eq!(parse_progress("  4% 12 - src/main.rs"), Some(4));
        assert_eq!(parse_progress(" 47%"), Some(47));
        assert_eq!(parse_progress("100% 3 - x"), Some(100));
    }

    #[test]
    fn ignora_linhas_sem_percentual() {
        assert_eq!(parse_progress("Everything is Ok"), None);
        assert_eq!(parse_progress(""), None);
        assert_eq!(parse_progress("arquivo 50porcento.txt"), None);
    }

    #[test]
    fn nao_confunde_percentual_no_nome_de_arquivo() {
        // O percentual válido é o primeiro token da linha.
        assert_eq!(parse_progress("- foto 100%.jpg"), None);
    }
}
