use anyhow::{Result, anyhow};
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread::{self, JoinHandle};

type Job = Box<dyn FnOnce() -> Result<()> + Send + 'static>;

enum Command {
    Job(Job),
    Flush(Sender<Option<String>>),
    Shutdown(Sender<()>),
}

/// Cloneable submission endpoint for asynchronous producers. The worker owner
/// remains responsible for flushing and shutting down the shared FIFO.
#[derive(Clone)]
pub struct PersistenceHandle {
    tx: SyncSender<Command>,
    errors: Arc<Mutex<Option<String>>>,
}

impl PersistenceHandle {
    pub fn enqueue<F>(&self, job: F) -> Result<()>
    where
        F: FnOnce() -> Result<()> + Send + 'static,
    {
        submit(&self.tx, &self.errors, Box::new(job))
    }
}

/// Ordered, single-flight persistence worker. Jobs are executed in submission
/// order, keeping the newest completed snapshot from being overwritten by an
/// older write. `flush` is the shutdown barrier used by the owner at exit.
pub struct PersistenceWorker {
    tx: SyncSender<Command>,
    thread: Option<JoinHandle<()>>,
    errors: Arc<Mutex<Option<String>>>,
}

impl PersistenceWorker {
    pub fn spawn(thread_name: &'static str) -> Result<Self> {
        let (tx, rx) = mpsc::sync_channel(128);
        let errors = Arc::new(Mutex::new(None));
        let worker_errors = errors.clone();
        let thread = thread::Builder::new()
            .name(thread_name.to_string())
            .spawn(move || run(rx, worker_errors))?;
        Ok(Self {
            tx,
            thread: Some(thread),
            errors,
        })
    }

    pub fn handle(&self) -> PersistenceHandle {
        PersistenceHandle {
            tx: self.tx.clone(),
            errors: self.errors.clone(),
        }
    }

    /// Observe the first background failure without consuming a flush barrier.
    pub fn pending_error(&self) -> Option<String> {
        self.errors.lock().clone()
    }

    pub fn enqueue<F>(&self, job: F) -> Result<()>
    where
        F: FnOnce() -> Result<()> + Send + 'static,
    {
        submit(&self.tx, &self.errors, Box::new(job))
    }

    pub fn flush(&self) -> Result<()> {
        let (done_tx, done_rx) = mpsc::channel();
        self.tx
            .send(Command::Flush(done_tx))
            .map_err(|_| anyhow!("persistence worker stopped"))?;
        let error = done_rx
            .recv()
            .map_err(|_| anyhow!("persistence worker stopped"))?;
        if let Some(error) = error {
            return Err(anyhow!("{error}"));
        }
        Ok(())
    }
}

impl Drop for PersistenceWorker {
    fn drop(&mut self) {
        let (done_tx, done_rx) = mpsc::channel();
        if self.tx.send(Command::Shutdown(done_tx)).is_ok() {
            let _ = done_rx.recv();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn submit(tx: &SyncSender<Command>, errors: &Arc<Mutex<Option<String>>>, job: Job) -> Result<()> {
    tx.try_send(Command::Job(job)).map_err(|error| {
        let message = format!("persistence queue unavailable: {error}");
        let mut pending = errors.lock();
        if pending.is_none() {
            *pending = Some(message.clone());
        }
        anyhow!(message)
    })
}

fn run(rx: Receiver<Command>, errors: Arc<Mutex<Option<String>>>) {
    while let Ok(command) = rx.recv() {
        match command {
            Command::Job(job) => {
                if let Err(error) = job() {
                    log::error!("persistence worker job failed: {error:#}");
                    let mut slot = errors.lock();
                    if slot.is_none() {
                        *slot = Some(format!("{error:#}"));
                    }
                }
            }
            Command::Flush(done) => {
                let error = errors.lock().take();
                let _ = done.send(error);
            }
            Command::Shutdown(done) => {
                let _ = done.send(());
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PersistenceWorker;
    use parking_lot::Mutex;
    use std::sync::Arc;

    #[test]
    fn flush_waits_for_ordered_jobs_and_reports_failures() {
        let worker = PersistenceWorker::spawn("persistence-test").unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let first = seen.clone();
        worker
            .enqueue(move || {
                first.lock().push(1);
                Ok(())
            })
            .unwrap();
        let second = seen.clone();
        worker
            .enqueue(move || {
                second.lock().push(2);
                Err(anyhow::anyhow!("expected failure"))
            })
            .unwrap();
        assert!(worker.flush().is_err());
        assert_eq!(*seen.lock(), vec![1, 2]);
    }
}
