use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use crate::{Engine, RenderOutcome, RenderRequest};

#[derive(Default)]
struct Slot {
    pending: Option<RenderRequest>,
    shutdown: bool,
}

/// Renderwarteschlange eines offenen Dokuments.
///
/// Höchstens ein Auftrag läuft, höchstens die jüngste angeforderte Revision
/// wartet. Ältere wartende Aufträge werden verworfen.
pub struct Renderer {
    shared: Arc<(Mutex<Slot>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}

impl Renderer {
    pub fn start<E: Engine>(engine: E) -> (Self, Receiver<RenderOutcome>) {
        let shared = Arc::new((Mutex::new(Slot::default()), Condvar::new()));
        let (tx, rx) = mpsc::channel();
        let worker = {
            let shared = Arc::clone(&shared);
            thread::Builder::new()
                .name("formavium-render".into())
                .spawn(move || run(engine, shared, tx))
                .expect("Renderthread startbar")
        };
        (
            Renderer {
                shared,
                worker: Some(worker),
            },
            rx,
        )
    }

    /// Fordert eine Revision an. Ein älterer wartender Auftrag wird ersetzt.
    pub fn request(&self, req: RenderRequest) {
        let (lock, cv) = &*self.shared;
        let mut slot = lock.lock().expect("Renderzustand");
        match &slot.pending {
            Some(p) if p.revision > req.revision => {}
            _ => slot.pending = Some(req),
        }
        cv.notify_one();
    }
}

fn run<E: Engine>(engine: E, shared: Arc<(Mutex<Slot>, Condvar)>, tx: Sender<RenderOutcome>) {
    let (lock, cv) = &*shared;
    let mut last_started = 0;
    loop {
        let req = {
            let mut slot = lock.lock().expect("Renderzustand");
            loop {
                if slot.shutdown {
                    return;
                }
                if let Some(r) = slot.pending.take() {
                    break r;
                }
                slot = cv.wait(slot).expect("Renderzustand");
            }
        };
        if req.revision < last_started {
            continue;
        }
        last_started = req.revision;
        if tx.send(engine.compile(&req)).is_err() {
            return;
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        {
            let (lock, cv) = &*self.shared;
            if let Ok(mut slot) = lock.lock() {
                slot.shutdown = true;
            }
            cv.notify_all();
        }
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
