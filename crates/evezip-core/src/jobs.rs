use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use evezip_engine::{CancelToken, CreateOptions, EngineError};

use crate::engine_trait::ArchiveEngine;

pub type JobId = u64;

#[derive(Debug, Clone)]
pub enum JobKind {
    Extract { archive: PathBuf, dest: PathBuf, entries: Option<Vec<String>>, password: Option<String> },
    Create { archive: PathBuf, inputs: Vec<PathBuf>, options: CreateOptions },
    Test { archive: PathBuf, password: Option<String> },
}

#[derive(Debug, Clone)]
pub struct JobSpec {
    pub descricao: String,
    pub kind: JobKind,
}

#[derive(Debug)]
pub enum JobEvent {
    Started(JobId),
    Progress(JobId, u8),
    Done(JobId),
    Failed(JobId, EngineError),
    Cancelled(JobId),
}

pub struct JobQueue {
    engine: Arc<dyn ArchiveEngine>,
    events: Sender<JobEvent>,
    next_id: AtomicU64,
    cancels: Arc<Mutex<HashMap<JobId, CancelToken>>>,
}

impl JobQueue {
    pub fn new(engine: Arc<dyn ArchiveEngine>, events: Sender<JobEvent>) -> Self {
        JobQueue {
            engine,
            events,
            next_id: AtomicU64::new(1),
            cancels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn submit(&self, spec: JobSpec) -> JobId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let cancel = CancelToken::new();
        self.cancels.lock().unwrap().insert(id, cancel.clone());

        let engine = Arc::clone(&self.engine);
        let events = self.events.clone();
        let cancels = Arc::clone(&self.cancels);

        std::thread::spawn(move || {
            let _ = events.send(JobEvent::Started(id));
            let ev = events.clone();
            let mut on_progress = move |p: u8| {
                let _ = ev.send(JobEvent::Progress(id, p));
            };

            let resultado = match &spec.kind {
                JobKind::Extract { archive, dest, entries, password } => engine.extract(
                    archive,
                    dest,
                    entries.as_deref(),
                    password.as_deref(),
                    &mut on_progress,
                    &cancel,
                ),
                JobKind::Create { archive, inputs, options } => {
                    engine.create(archive, inputs, options, &mut on_progress, &cancel)
                }
                JobKind::Test { archive, password } => {
                    engine.test(archive, password.as_deref(), &mut on_progress, &cancel)
                }
            };

            cancels.lock().unwrap().remove(&id);
            let _ = match resultado {
                Ok(()) => events.send(JobEvent::Done(id)),
                Err(EngineError::Cancelado) => events.send(JobEvent::Cancelled(id)),
                Err(e) => events.send(JobEvent::Failed(id, e)),
            };
        });

        id
    }

    pub fn cancel(&self, id: JobId) {
        if let Some(tok) = self.cancels.lock().unwrap().get(&id) {
            tok.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::time::Duration;
    use evezip_engine::{ArchiveEntry, CancelToken, CreateOptions, EngineError};
    use crate::engine_trait::ArchiveEngine;

    /// Engine falso: extract emite 3 progressos; respeita cancelamento; test falha.
    struct EngineFalso {
        lento: AtomicBool,
    }

    impl ArchiveEngine for EngineFalso {
        fn list(&self, _: &Path, _: Option<&str>) -> Result<Vec<ArchiveEntry>, EngineError> {
            Ok(vec![])
        }
        fn extract(
            &self, _: &Path, _: &Path, _: Option<&[String]>, _: Option<&str>,
            on_progress: &mut dyn FnMut(u8), cancel: &CancelToken,
        ) -> Result<(), EngineError> {
            for p in [10u8, 50, 100] {
                if self.lento.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(50));
                }
                if cancel.is_cancelled() {
                    return Err(EngineError::Cancelado);
                }
                on_progress(p);
            }
            Ok(())
        }
        fn create(
            &self, _: &Path, _: &[std::path::PathBuf], _: &CreateOptions,
            _: &mut dyn FnMut(u8), _: &CancelToken,
        ) -> Result<(), EngineError> {
            Ok(())
        }
        fn test(
            &self, _: &Path, _: Option<&str>, _: &mut dyn FnMut(u8), _: &CancelToken,
        ) -> Result<(), EngineError> {
            Err(EngineError::ArchiveCorrompido("CRC".into()))
        }
    }

    fn fila(lento: bool) -> (JobQueue, mpsc::Receiver<JobEvent>) {
        let (tx, rx) = mpsc::channel();
        let q = JobQueue::new(Arc::new(EngineFalso { lento: AtomicBool::new(lento) }), tx);
        (q, rx)
    }

    fn spec_extract() -> JobSpec {
        JobSpec {
            descricao: "extrair a.7z".into(),
            kind: JobKind::Extract {
                archive: "/x/a.7z".into(),
                dest: "/x/out".into(),
                entries: None,
                password: None,
            },
        }
    }

    #[test]
    fn job_bem_sucedido_emite_started_progress_done() {
        let (q, rx) = fila(false);
        let id = q.submit(spec_extract());
        let evs: Vec<JobEvent> = rx.iter().take(5).collect();
        assert!(matches!(evs[0], JobEvent::Started(i) if i == id));
        assert!(matches!(evs[1], JobEvent::Progress(_, 10)));
        assert!(matches!(evs[4], JobEvent::Done(_)));
    }

    #[test]
    fn job_com_erro_emite_failed() {
        let (q, rx) = fila(false);
        q.submit(JobSpec {
            descricao: "testar".into(),
            kind: JobKind::Test { archive: "/x/a.7z".into(), password: None },
        });
        let evs: Vec<JobEvent> = rx.iter().take(2).collect();
        assert!(matches!(evs[1], JobEvent::Failed(_, EngineError::ArchiveCorrompido(_))));
    }

    #[test]
    fn cancelamento_emite_cancelled() {
        let (q, rx) = fila(true);
        let id = q.submit(spec_extract());
        assert!(matches!(rx.recv().unwrap(), JobEvent::Started(_)));
        q.cancel(id);
        let ultimo = rx.iter().find(|e| matches!(e, JobEvent::Cancelled(_) | JobEvent::Done(_)));
        assert!(matches!(ultimo, Some(JobEvent::Cancelled(_))));
    }

    #[test]
    fn dois_jobs_concorrentes_completam() {
        let (q, rx) = fila(false);
        q.submit(spec_extract());
        q.submit(spec_extract());
        let dones = rx.iter().take(10).filter(|e| matches!(e, JobEvent::Done(_))).count();
        assert_eq!(dones, 2);
    }
}
