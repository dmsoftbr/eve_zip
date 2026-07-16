use std::collections::BTreeMap;

use evezip_engine::ArchiveEntry;

use crate::browser::Row;

#[derive(Debug, Default)]
struct Node {
    is_dir: bool,
    size: u64,
    packed_size: u64,
    modified: String,
    children: BTreeMap<String, Node>,
}

#[derive(Debug, Default)]
pub struct ArchiveTree {
    root: Node,
}

impl ArchiveTree {
    pub fn build(entries: &[ArchiveEntry]) -> Self {
        let mut root = Node { is_dir: true, ..Node::default() };
        for e in entries {
            let caminho = e.path.replace('\\', "/");
            let partes: Vec<&str> = caminho.split('/').filter(|p| !p.is_empty()).collect();
            let mut atual = &mut root;
            for (i, parte) in partes.iter().enumerate() {
                let ultimo = i == partes.len() - 1;
                let filho = atual.children.entry((*parte).to_string()).or_insert_with(|| Node {
                    is_dir: true, // implícito até provar o contrário
                    ..Node::default()
                });
                if ultimo {
                    // Um nó que já tem filhos (ex.: "a/b.txt" processado antes de "a")
                    // é sempre diretório, mesmo que a entrada explícita diga is_dir=false.
                    filho.is_dir = e.is_dir || !filho.children.is_empty();
                    filho.size = e.size;
                    filho.packed_size = e.packed_size;
                    filho.modified = e.modified.clone().unwrap_or_default();
                } else {
                    // Estamos descendo através deste nó para chegar a um filho:
                    // ele precisa ser diretório, independentemente de entradas futuras.
                    filho.is_dir = true;
                }
                atual = filho;
            }
        }
        ArchiveTree { root }
    }

    pub fn list_dir(&self, inner_path: &str) -> Option<Vec<Row>> {
        let mut node = &self.root;
        for parte in inner_path.split('/').filter(|p| !p.is_empty()) {
            node = node.children.get(parte)?;
        }
        let mut rows: Vec<Row> = node
            .children
            .iter()
            .map(|(nome, n)| Row {
                name: nome.clone(),
                size: n.size,
                packed_size: n.packed_size,
                modified: n.modified.clone(),
                is_dir: n.is_dir,
            })
            .collect();
        rows.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
        Some(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evezip_engine::ArchiveEntry;

    fn e(path: &str, is_dir: bool, size: u64) -> ArchiveEntry {
        ArchiveEntry {
            path: path.into(),
            size,
            packed_size: size / 2,
            modified: Some("2026-07-01 10:00:00".into()),
            is_dir,
            encrypted: false,
        }
    }

    #[test]
    fn lista_raiz_com_dirs_primeiro() {
        let t = ArchiveTree::build(&[
            e("zeta.txt", false, 10),
            e("src", true, 0),
            e("src/main.rs", false, 20),
        ]);
        let rows = t.list_dir("").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "src");
        assert!(rows[0].is_dir);
        assert_eq!(rows[1].name, "zeta.txt");
    }

    #[test]
    fn lista_subdiretorio() {
        let t = ArchiveTree::build(&[e("src", true, 0), e("src/main.rs", false, 20)]);
        let rows = t.list_dir("src").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "main.rs");
        assert_eq!(rows[0].size, 20);
    }

    #[test]
    fn cria_diretorios_intermediarios_implicitos() {
        // Alguns zips não têm entrada para o diretório, só para o arquivo.
        let t = ArchiveTree::build(&[e("a/b/c.txt", false, 5)]);
        let raiz = t.list_dir("").unwrap();
        assert_eq!(raiz[0].name, "a");
        assert!(raiz[0].is_dir);
        let b = t.list_dir("a/b").unwrap();
        assert_eq!(b[0].name, "c.txt");
    }

    #[test]
    fn caminho_inexistente_retorna_none() {
        let t = ArchiveTree::build(&[e("a.txt", false, 1)]);
        assert!(t.list_dir("nao/existe").is_none());
    }

    #[test]
    fn normaliza_separador_windows() {
        let t = ArchiveTree::build(&[e("src\\main.rs", false, 20)]);
        assert!(t.list_dir("src").is_some());
    }

    #[test]
    fn arquivo_que_tambem_e_diretorio_vira_diretorio() {
        // Alguns zips têm uma entrada "a" (arquivo) e também "a/b.txt".
        // O nó "a" precisa virar diretório para que b.txt continue acessível.
        for entradas in [
            vec![e("a", false, 5), e("a/b.txt", false, 3)],
            vec![e("a/b.txt", false, 3), e("a", false, 5)],
        ] {
            let t = ArchiveTree::build(&entradas);
            let raiz = t.list_dir("").unwrap();
            assert_eq!(raiz.len(), 1);
            assert_eq!(raiz[0].name, "a");
            assert!(raiz[0].is_dir, "nó com filhos deve ser tratado como diretório");
            let dentro = t.list_dir("a").unwrap();
            assert_eq!(dentro.len(), 1);
            assert_eq!(dentro[0].name, "b.txt");
        }
    }
}
