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
                    filho.is_dir = e.is_dir;
                    filho.size = e.size;
                    filho.packed_size = e.packed_size;
                    filho.modified = e.modified.clone().unwrap_or_default();
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
}
