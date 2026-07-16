pub fn tamanho_humano(bytes: u64) -> String {
    const UNIDADES: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut v = bytes as f64;
    let mut unidade = "";
    for u in UNIDADES {
        v /= 1024.0;
        unidade = u;
        if v < 1024.0 {
            break;
        }
    }
    format!("{:.1} {}", v, unidade).replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formata() {
        assert_eq!(tamanho_humano(0), "0 B");
        assert_eq!(tamanho_humano(999), "999 B");
        assert_eq!(tamanho_humano(1024), "1,0 KB");
        assert_eq!(tamanho_humano(14 * 1024), "14,0 KB");
        assert_eq!(tamanho_humano(5 * 1024 * 1024), "5,0 MB");
        assert_eq!(tamanho_humano(3 * 1024 * 1024 * 1024), "3,0 GB");
    }
}
