use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

pub(crate) struct ShutdownManager {
    pub(crate) shutdown_tx: broadcast::Sender<()>,
    pub(crate) _shutdown_rcv: broadcast::Receiver<()>,
    pub(crate) external_shutdown: mpsc::UnboundedReceiver<()>,
    handles: Vec<JoinHandle<()>>,
}

pub(crate) struct Shutdown {
    pub(crate) shutdown_signal_rcv: broadcast::Receiver<()>,
}

#[derive(Clone)]
pub struct ShutdownHandle {
    pub(crate) external_shutdown: mpsc::UnboundedSender<()>,
    pub(crate) shutdown_started: bool,
}

impl ShutdownHandle {
    pub fn shutdown(&mut self) {
        self.shutdown_started = true;
        self.external_shutdown.send(()).unwrap();
    }
}

impl ShutdownManager {
    pub(crate) fn init() -> (ShutdownManager, ShutdownHandle) {
        let (shutdown_tx, shutdown_rcv) = broadcast::channel(1);
        let (external_tx, external_rcv) = mpsc::unbounded_channel();
        let shutdown_handle = ShutdownHandle {
            external_shutdown: external_tx,
            shutdown_started: false,
        };
        let manager = Self {
            shutdown_tx,
            _shutdown_rcv: shutdown_rcv,
            external_shutdown: external_rcv,
            handles: vec![],
        };
        (manager, shutdown_handle)
    }

    pub async fn stop(self) {
        log::info!("Starting Ephemera shutdown");
        self.shutdown_tx.send(()).unwrap();
        log::info!("Waiting for tasks to finish");
        for handle in self.handles.into_iter() {
            handle.await.unwrap();
        }
    }

    pub(crate) fn subscribe(&self) -> Shutdown {
        let shutdown = self.shutdown_tx.subscribe();
        Shutdown {
            shutdown_signal_rcv: shutdown,
        }
    }

    pub(crate) fn add_handle(&mut self, handle: JoinHandle<()>) {
        self.handles.push(handle);
    }
}
