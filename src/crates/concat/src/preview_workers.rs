// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Keep interactive frame work independent of background media jobs.

use std::sync::{Arc, Mutex, mpsc};

type Job = Box<dyn FnOnce() + Send + 'static>;

/// A bounded number of workers, with a separate queue for monitor frames.
pub struct PreviewWorkers {
    background: mpsc::Sender<Job>,
    frames: mpsc::Sender<Job>,
}

impl PreviewWorkers {
    /// Start `background_threads` background workers and one frame worker.
    pub fn new(background_threads: usize) -> Self {
        assert!(background_threads > 0);
        Self {
            background: queue("concat-worker", background_threads),
            frames: queue("concat-preview", 1),
        }
    }

    /// Submit analysis, thumbnails, prefetch or other background work.
    pub fn background(&self, work: impl FnOnce() + Send + 'static) {
        let _ = self.background.send(Box::new(work));
    }

    /// Submit a monitor frame without waiting behind background work.
    pub fn frame(&self, work: impl FnOnce() + Send + 'static) {
        let _ = self.frames.send(Box::new(work));
    }
}

fn queue(name: &str, count: usize) -> mpsc::Sender<Job> {
    let (sender, receiver) = mpsc::channel::<Job>();
    let receiver = Arc::new(Mutex::new(receiver));
    for index in 0..count {
        let receiver = Arc::clone(&receiver);
        std::thread::Builder::new()
            .name(format!("{name}-{index}"))
            .spawn(move || {
                loop {
                    let job = receiver.lock().unwrap_or_else(|e| e.into_inner()).recv();
                    match job {
                        Ok(job) => job(),
                        Err(_) => return,
                    }
                }
            })
            .expect("a media worker thread");
    }
    sender
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const DEADLINE: Duration = Duration::from_secs(5);

    #[test]
    fn frame_finishes_while_all_background_workers_are_blocked() {
        let workers = PreviewWorkers::new(3);
        let mut release = Vec::new();
        for _ in 0..3 {
            let (started, running) = mpsc::channel();
            let (resume, wait) = mpsc::channel::<()>();
            workers.background(move || {
                started.send(()).unwrap();
                // A dropped sender releases the worker even if an assertion fails.
                let _ = wait.recv();
            });
            release.push(resume);
            running
                .recv_timeout(DEADLINE)
                .expect("background job started");
        }
        let (background_done, background) = mpsc::channel();
        workers.background(move || background_done.send(()).unwrap());
        let (frame_done, frame) = mpsc::channel();
        workers.frame(move || frame_done.send(()).unwrap());
        frame
            .recv_timeout(DEADLINE)
            .expect("frame bypasses busy background workers");
        assert!(matches!(
            background.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        drop(release);
        background
            .recv_timeout(DEADLINE)
            .expect("background queue drains too");
    }

    #[test]
    fn slow_frame_does_not_block_background_work() {
        let workers = PreviewWorkers::new(1);
        let (started, running) = mpsc::channel();
        let (resume, wait) = mpsc::channel::<()>();
        workers.frame(move || {
            started.send(()).unwrap();
            let _ = wait.recv();
        });
        running.recv_timeout(DEADLINE).expect("frame started");
        let (done, result) = mpsc::channel();
        workers.background(move || done.send(()).unwrap());
        result
            .recv_timeout(DEADLINE)
            .expect("background work remains live");
        drop(resume);
    }
}
