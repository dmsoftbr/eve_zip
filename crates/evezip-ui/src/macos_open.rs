//! Tratamento do Apple Event `kAEOpenDocuments` no macOS.
//!
//! No macOS, dar duplo-clique num arquivo no Finder (ou rodar
//! `open -a EveZip arquivo.zip`) NÃO passa o caminho por `argv`: o
//! LaunchServices envia um Apple Event `aevt/odoc` para o app. O delegate do
//! AppKit padrão (usado pelo winit, que o Slint usa) não trata esse evento e
//! exibe o diálogo "cannot open files in the Archive format".
//!
//! A correção é instalar nosso próprio handler para `kAEOpenDocuments`,
//! sobrescrevendo o do AppKit. Ordem importa: dentro de `finishLaunching` o
//! AppKit (1) registra os handlers de Apple Event, (2) posta
//! `NSApplicationWillFinishLaunchingNotification`, (3) despacha o `odoc`
//! enfileirado do lançamento, (4) posta `...DidFinishLaunching`. Instalamos o
//! nosso handler no observador de (2) — sobrescreve (1) e antecede (3). Usar
//! `Did` seria tarde demais (verificado: o `odoc` de lançamento não chegava).
//!
//! Os caminhos recebidos são acumulados em `PENDING`; o `main.rs` drena essa
//! fila por um `slint::Timer` no thread da UI e navega até o archive.

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::Mutex;

use block2::RcBlock;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSString};

static PENDING: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Drena os caminhos recebidos desde a última chamada.
pub fn take_pending() -> Vec<PathBuf> {
    match PENDING.lock() {
        Ok(mut q) => std::mem::take(&mut *q),
        Err(_) => Vec::new(),
    }
}

// --- FFI mínimo do Apple Event Manager (framework CoreServices) ---

type OSErr = i16;
type AEEventClass = u32;
type AEEventID = u32;
type AEKeyword = u32;
type DescType = u32;

#[repr(C)]
struct AEDesc {
    descriptor_type: DescType,
    data_handle: *mut c_void,
}

type AppleEvent = AEDesc;
type AEDescList = AEDesc;
type AEEventHandlerProcPtr = extern "C" fn(*const AppleEvent, *mut AppleEvent, isize) -> OSErr;

const fn fourcc(b: &[u8; 4]) -> u32 {
    ((b[0] as u32) << 24) | ((b[1] as u32) << 16) | ((b[2] as u32) << 8) | (b[3] as u32)
}

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn AEInstallEventHandler(
        the_ae_event_class: AEEventClass,
        the_ae_event_id: AEEventID,
        handler: AEEventHandlerProcPtr,
        handler_refcon: isize,
        is_sys_handler: u8,
    ) -> OSErr;
    fn AEGetParamDesc(
        the_apple_event: *const AppleEvent,
        the_ae_keyword: AEKeyword,
        desired_type: DescType,
        result: *mut AEDesc,
    ) -> OSErr;
    fn AECountItems(the_ae_desc_list: *const AEDescList, the_count: *mut isize) -> OSErr;
    fn AEGetNthPtr(
        the_ae_desc_list: *const AEDescList,
        index: isize,
        desired_type: DescType,
        the_ae_keyword: *mut AEKeyword,
        type_code: *mut DescType,
        data_ptr: *mut c_void,
        maximum_size: isize,
        actual_size: *mut isize,
    ) -> OSErr;
    fn AEDisposeDesc(the_ae_desc: *mut AEDesc) -> OSErr;
}

/// Handler C do `odoc`: extrai os caminhos (typeFileURL) e os empurra em `PENDING`.
extern "C" fn handle_open_documents(
    event: *const AppleEvent,
    _reply: *mut AppleEvent,
    _refcon: isize,
) -> OSErr {
    const KEY_DIRECT_OBJECT: AEKeyword = fourcc(b"----");
    const TYPE_AE_LIST: DescType = fourcc(b"list");
    const TYPE_FILE_URL: DescType = fourcc(b"furl");

    let mut caminhos = Vec::new();
    unsafe {
        let mut lista = AEDesc {
            descriptor_type: 0,
            data_handle: std::ptr::null_mut(),
        };
        if AEGetParamDesc(event, KEY_DIRECT_OBJECT, TYPE_AE_LIST, &mut lista) != 0 {
            return 0;
        }
        let mut count: isize = 0;
        AECountItems(&lista, &mut count);
        for i in 1..=count {
            let mut buf = vec![0u8; 4096];
            let mut kw: AEKeyword = 0;
            let mut tc: DescType = 0;
            let mut actual: isize = 0;
            let err = AEGetNthPtr(
                &lista,
                i,
                TYPE_FILE_URL,
                &mut kw,
                &mut tc,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as isize,
                &mut actual,
            );
            if err == 0 && actual > 0 {
                buf.truncate(actual as usize);
                if let Some(p) = file_url_bytes_para_path(&buf) {
                    caminhos.push(p);
                }
            }
        }
        AEDisposeDesc(&mut lista);
    }

    if !caminhos.is_empty() {
        if let Ok(mut q) = PENDING.lock() {
            q.extend(caminhos);
        }
    }
    0
}

/// Converte os bytes de uma file URL (`file:///Users/...`) em `PathBuf`,
/// desfazendo o percent-encoding.
fn file_url_bytes_para_path(bytes: &[u8]) -> Option<PathBuf> {
    let s = std::str::from_utf8(bytes).ok()?;
    let sem_esquema = s.strip_prefix("file://")?;
    // Remove uma eventual autoridade vazia deixando o caminho absoluto ("/...").
    let caminho_codificado = sem_esquema;
    let decodificado = percent_decode(caminho_codificado);
    let texto = String::from_utf8(decodificado).ok()?;
    if texto.is_empty() {
        None
    } else {
        Some(PathBuf::from(texto))
    }
}

fn percent_decode(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// Instala o handler do `odoc`, no momento certo (após o AppKit registrar o
/// dele). Chamar UMA vez, antes de `run()`.
pub fn install() {
    const K_CORE_EVENT_CLASS: AEEventClass = fourcc(b"aevt");
    const K_AE_OPEN_DOCUMENTS: AEEventID = fourcc(b"odoc");

    // Bloco disparado por NSApplicationWillFinishLaunchingNotification: instala
    // nosso handler, sobrescrevendo o do AppKit. A ordem dentro de
    // `-[NSApplication finishLaunching]` é: (1) AppKit registra os handlers de
    // Apple Event, (2) posta *Will*FinishLaunching, (3) despacha o `odoc`
    // enfileirado do lançamento, (4) posta *Did*FinishLaunching. Instalar em
    // (2) sobrescreve (1) e antecede o despacho em (3) — `Did` seria tarde
    // demais (o `odoc` do lançamento já teria ido para o AppKit).
    let bloco = RcBlock::new(move |_notif: std::ptr::NonNull<NSNotification>| unsafe {
        AEInstallEventHandler(
            K_CORE_EVENT_CLASS,
            K_AE_OPEN_DOCUMENTS,
            handle_open_documents,
            0,
            0,
        );
    });

    unsafe {
        let center = NSNotificationCenter::defaultCenter();
        let nome = NSString::from_str("NSApplicationWillFinishLaunchingNotification");
        let token =
            center.addObserverForName_object_queue_usingBlock(Some(&nome), None, None, &bloco);
        // O observador precisa viver enquanto o app viver; como só o
        // instalamos uma vez, vaza-se o token de propósito.
        std::mem::forget(token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodifica_file_url_simples() {
        let p =
            file_url_bytes_para_path(b"file:///Users/diogo/Downloads/chrome-qa-loop.zip").unwrap();
        assert_eq!(
            p,
            PathBuf::from("/Users/diogo/Downloads/chrome-qa-loop.zip")
        );
    }

    #[test]
    fn decodifica_espacos_e_acentos() {
        // "pasta com espaço/relatório.7z" percent-encoded.
        let p = file_url_bytes_para_path(
            "file:///Users/diogo/pasta%20com%20espa%C3%A7o/relat%C3%B3rio.7z".as_bytes(),
        )
        .unwrap();
        assert_eq!(
            p,
            PathBuf::from("/Users/diogo/pasta com espaço/relatório.7z")
        );
    }

    #[test]
    fn rejeita_sem_esquema_file() {
        assert!(file_url_bytes_para_path(b"/Users/diogo/x.zip").is_none());
        assert!(file_url_bytes_para_path(b"http://exemplo/x.zip").is_none());
    }

    #[test]
    fn percent_decode_percentual_invalido_fica_literal() {
        // "%zz" não é hex válido: mantém-se literal, sem panicar.
        assert_eq!(percent_decode("a%zzb"), b"a%zzb");
        // "%2" truncado no fim: literal.
        assert_eq!(percent_decode("fim%2"), b"fim%2");
    }
}
