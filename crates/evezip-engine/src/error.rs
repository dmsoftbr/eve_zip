use std::fmt;

#[derive(Debug)]
pub enum EngineError {
    BinarioNaoEncontrado,
    SenhaNecessaria,
    SenhaIncorreta,
    ArchiveCorrompido(String),
    SemEspacoEmDisco,
    Cancelado,
    Io(std::io::Error),
    Falha { exit_code: i32, stderr: String },
    OpcaoNaoSuportada(String),
    CaminhoInvalido(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinarioNaoEncontrado => write!(f, "binário 7zz não encontrado"),
            Self::SenhaNecessaria => write!(f, "o archive exige senha"),
            Self::SenhaIncorreta => write!(f, "senha incorreta"),
            Self::ArchiveCorrompido(d) => write!(f, "archive corrompido: {d}"),
            Self::SemEspacoEmDisco => write!(f, "sem espaço em disco"),
            Self::Cancelado => write!(f, "operação cancelada"),
            Self::Io(e) => write!(f, "erro de E/S: {e}"),
            Self::Falha { exit_code, stderr } => {
                write!(f, "7zz falhou (código {exit_code}): {stderr}")
            }
            Self::OpcaoNaoSuportada(d) => write!(f, "opção não suportada: {d}"),
            Self::CaminhoInvalido(d) => write!(f, "caminho inválido no archive: {d}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<std::io::Error> for EngineError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
