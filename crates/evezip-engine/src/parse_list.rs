use crate::entry::ArchiveEntry;

/// Parseia a saída de `7zz l -slt <archive>`.
/// Entradas vêm após a linha "----------", em blocos "Chave = Valor"
/// separados por linha em branco.
pub fn parse_slt(output: &str) -> Vec<ArchiveEntry> {
    let output = output.replace("\r\n", "\n");
    let corpo = match output.split("----------").nth(1) {
        Some(c) => c,
        None => return Vec::new(),
    };

    let mut entradas = Vec::new();
    for bloco in corpo.split("\n\n") {
        let mut path = None;
        let mut size = 0u64;
        let mut packed = 0u64;
        let mut modified = None;
        let mut is_dir = false;
        let mut encrypted = false;

        for linha in bloco.lines() {
            let Some((chave, valor)) = linha.split_once(" = ") else {
                continue;
            };
            match chave.trim() {
                "Path" => path = Some(valor.to_string()),
                "Size" => size = valor.trim().parse().unwrap_or(0),
                "Packed Size" => packed = valor.trim().parse().unwrap_or(0),
                "Modified" => {
                    let v = valor.trim();
                    if !v.is_empty() {
                        modified = Some(v.to_string());
                    }
                }
                "Folder" => is_dir = is_dir || valor.trim() == "+",
                "Attributes" => {
                    is_dir = is_dir || valor.trim().starts_with('D');
                }
                "Encrypted" => encrypted = valor.trim() == "+",
                _ => {}
            }
        }

        if let Some(path) = path {
            entradas.push(ArchiveEntry {
                path,
                size,
                packed_size: packed,
                modified,
                is_dir,
                encrypted,
            });
        }
    }
    entradas
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAIDA_7Z: &str = "\
7-Zip (z) 25.01 (arm64)

Listing archive: test.7z

--
Path = test.7z
Type = 7z
Method = LZMA2

----------
Path = src
Folder = +
Size = 0
Packed Size = 0
Modified = 2026-07-01 10:00:00
Attributes = D drwxr-xr-x

Path = src/main.rs
Folder = -
Size = 1234
Packed Size = 600
Modified = 2026-07-02 08:30:11
Attributes = A -rw-r--r--
Encrypted = -

Path = src/segrêdo çedilha.txt
Folder = -
Size = 42
Packed Size = 48
Modified = 2026-07-03 23:59:59
Encrypted = +
";

    #[test]
    fn parseia_diretorio_arquivo_e_unicode() {
        let e = parse_slt(SAIDA_7Z);
        assert_eq!(e.len(), 3);

        assert_eq!(e[0].path, "src");
        assert!(e[0].is_dir);

        assert_eq!(e[1].path, "src/main.rs");
        assert!(!e[1].is_dir);
        assert_eq!(e[1].size, 1234);
        assert_eq!(e[1].packed_size, 600);
        assert_eq!(e[1].modified.as_deref(), Some("2026-07-02 08:30:11"));
        assert!(!e[1].encrypted);

        assert_eq!(e[2].path, "src/segrêdo çedilha.txt");
        assert!(e[2].encrypted);
    }

    #[test]
    fn detecta_diretorio_por_attributes_quando_nao_ha_folder() {
        // Listagem de .zip às vezes só traz Attributes com D.
        let saida = "\
----------
Path = docs
Size = 0
Packed Size = 0
Attributes = D
";
        let e = parse_slt(saida);
        assert_eq!(e.len(), 1);
        assert!(e[0].is_dir);
    }

    #[test]
    fn campos_ausentes_viram_default() {
        let saida = "\
----------
Path = a.txt
";
        let e = parse_slt(saida);
        assert_eq!(e[0].size, 0);
        assert_eq!(e[0].modified, None);
        assert!(!e[0].is_dir);
        assert!(!e[0].encrypted);
    }

    #[test]
    fn ignora_cabecalho_antes_do_separador() {
        assert!(parse_slt("7-Zip banner\nPath = nao-e-entrada\n").is_empty());
    }

    #[test]
    fn aceita_saida_com_crlf() {
        let saida = "----------\r\nPath = src\r\nFolder = +\r\nSize = 0\r\nPacked Size = 0\r\n\r\nPath = src/main.rs\r\nFolder = -\r\nSize = 1234\r\nPacked Size = 600\r\n";
        let e = parse_slt(saida);
        assert_eq!(e.len(), 2);

        assert_eq!(e[0].path, "src");
        assert!(e[0].is_dir);
        assert_eq!(e[0].size, 0);

        assert_eq!(e[1].path, "src/main.rs");
        assert!(!e[1].is_dir);
        assert_eq!(e[1].size, 1234);
    }
}
