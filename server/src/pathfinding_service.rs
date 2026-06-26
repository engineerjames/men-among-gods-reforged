//! Worker-thread pathfinding service.
//!
//! The main tick thread owns `GameState`; this service accepts copied request
//! data plus compact passability snapshots and computes path directions on a
//! small pool of worker threads.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

use crate::path_finding::{PathFinder, PathFindingRequest, PathFindingStats, SnapshotPassability};

/// Environment variable that enables asynchronous pathfinding when set.
pub const ENABLE_ASYNC_PATHFINDING_ENV: &str = "MAG_ASYNC_PATHFINDING";

const SNAPSHOT_MARGIN: usize = 16;
const DEFAULT_WORKER_COUNT: usize = 2;
const MAX_WORKER_COUNT: usize = 4;

/// Request submitted to pathfinding workers.
pub struct PathfindingJob {
    /// Monotonic request identifier assigned by the service.
    pub request_id: u64,
    /// Character slot this request belongs to.
    pub character_id: usize,
    /// Copied pathfinding inputs.
    pub request: PathFindingRequest,
    /// Compact passability snapshot for this request.
    pub passability: SnapshotPassability,
}

/// Completed pathfinding result from a worker.
#[derive(Clone, Copy, Debug)]
pub struct PathfindingResponse {
    /// Monotonic request identifier assigned by the service.
    pub request_id: u64,
    /// Character slot this request belongs to.
    pub character_id: usize,
    /// Copied pathfinding inputs used by the worker.
    pub request: PathFindingRequest,
    /// Direction selected by A*, or `None` if no path was found.
    pub direction: Option<u8>,
    /// Worker-local metrics for this request.
    pub stats: PathFindingStats,
}

enum WorkerMessage {
    Job(PathfindingJob),
    Shutdown,
}

/// State for one in-flight pathfinding request on the tick thread.
#[derive(Clone, Copy, Debug)]
pub struct PendingPathRequest {
    /// Monotonic request identifier assigned by the service.
    pub request_id: u64,
    /// Copied pathfinding inputs sent to the worker.
    pub request: PathFindingRequest,
}

/// Handle for a small pool of pathfinding worker threads.
pub struct PathfindingService {
    job_senders: Vec<Sender<WorkerMessage>>,
    result_rx: Receiver<PathfindingResponse>,
    handles: Vec<JoinHandle<()>>,
    next_request_id: u64,
    next_worker: usize,
    pending_by_character: HashMap<usize, PendingPathRequest>,
    completed_by_character: HashMap<usize, PathfindingResponse>,
}

impl PathfindingService {
    /// Spawn the worker pool when async pathfinding is enabled by environment.
    ///
    /// # Returns
    ///
    /// * `Some(PathfindingService)` when enabled.
    /// * `None` when `MAG_ASYNC_PATHFINDING` is unset, empty, `0`, or `false`.
    ///
    /// # Panics
    ///
    /// * Panics if an enabled worker thread cannot be spawned.
    pub fn spawn_from_env() -> Option<Self> {
        if !env_flag_enabled(ENABLE_ASYNC_PATHFINDING_ENV) {
            return None;
        }

        let worker_count = std::thread::available_parallelism()
            .map(|count| count.get().saturating_sub(1).clamp(1, MAX_WORKER_COUNT))
            .unwrap_or(DEFAULT_WORKER_COUNT);

        Some(Self::spawn(worker_count))
    }

    /// Spawn a fixed number of pathfinding worker threads.
    ///
    /// # Arguments
    ///
    /// * `worker_count` - Number of worker threads to spawn.
    ///
    /// # Returns
    ///
    /// * A service handle for request submission and result draining.
    ///
    /// # Panics
    ///
    /// * Panics if `worker_count` is zero or a worker thread cannot be spawned.
    pub fn spawn(worker_count: usize) -> Self {
        assert!(
            worker_count > 0,
            "pathfinding worker count must be non-zero"
        );

        let (result_tx, result_rx) = mpsc::channel::<PathfindingResponse>();
        let mut job_senders = Vec::with_capacity(worker_count);
        let mut handles = Vec::with_capacity(worker_count);

        for worker_index in 0..worker_count {
            let (job_tx, job_rx) = mpsc::channel::<WorkerMessage>();
            let worker_result_tx = result_tx.clone();
            let handle = thread::Builder::new()
                .name(format!("pathfinder-{worker_index}"))
                .spawn(move || worker_main(job_rx, worker_result_tx))
                .expect("Failed to spawn pathfinding worker thread");
            job_senders.push(job_tx);
            handles.push(handle);
        }

        Self {
            job_senders,
            result_rx,
            handles,
            next_request_id: 1,
            next_worker: 0,
            pending_by_character: HashMap::new(),
            completed_by_character: HashMap::new(),
        }
    }

    /// Return whether the character already has an in-flight request.
    ///
    /// # Arguments
    ///
    /// * `character_id` - Character slot to check.
    ///
    /// # Returns
    ///
    /// * `true` when a worker request is already pending.
    pub fn has_pending(&self, character_id: usize) -> bool {
        self.pending_by_character.contains_key(&character_id)
    }

    /// Return the most recent completed response for a character, if any.
    ///
    /// # Arguments
    ///
    /// * `character_id` - Character slot to look up.
    ///
    /// # Returns
    ///
    /// * `Some(response)` when a completed response is waiting.
    pub fn completed_for(&self, character_id: usize) -> Option<PathfindingResponse> {
        self.completed_by_character.get(&character_id).copied()
    }

    /// Remove a completed response for a character.
    ///
    /// # Arguments
    ///
    /// * `character_id` - Character slot whose response should be removed.
    pub fn discard_completed(&mut self, character_id: usize) {
        self.completed_by_character.remove(&character_id);
    }

    /// Submit a request if the character has no equivalent pending request.
    ///
    /// # Arguments
    ///
    /// * `character_id` - Character slot this request belongs to.
    /// * `request` - Copied pathfinding request.
    /// * `map` - Live world map used to build a compact snapshot.
    /// * `items` - Live item table used to build a compact snapshot.
    ///
    /// # Returns
    ///
    /// * `true` if a new request was enqueued.
    /// * `false` if an equivalent request is already pending or enqueue failed.
    pub fn submit_if_absent(
        &mut self,
        character_id: usize,
        request: PathFindingRequest,
        map: &[core::types::Map],
        items: &[core::types::Item],
    ) -> bool {
        if let Some(pending) = self.pending_by_character.get(&character_id)
            && pending.request == request
        {
            return false;
        }

        self.completed_by_character.remove(&character_id);
        self.pending_by_character.remove(&character_id);

        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        let passability = SnapshotPassability::from_world_window(
            map,
            items,
            request.character.mapblock(),
            &request,
            SNAPSHOT_MARGIN,
        );
        let job = PathfindingJob {
            request_id,
            character_id,
            request,
            passability,
        };

        let worker_index = self.next_worker % self.job_senders.len();
        self.next_worker = (self.next_worker + 1) % self.job_senders.len();
        if let Err(error) = self.job_senders[worker_index].send(WorkerMessage::Job(job)) {
            log::error!("Failed to enqueue pathfinding job: {error}");
            return false;
        }

        self.pending_by_character.insert(
            character_id,
            PendingPathRequest {
                request_id,
                request,
            },
        );
        true
    }

    /// Drain all completed worker responses into the service cache.
    ///
    /// # Returns
    ///
    /// * Aggregated worker pathfinding metrics drained this tick.
    pub fn drain_completed(&mut self) -> PathFindingStats {
        let mut stats = PathFindingStats::default();
        let mut completed_request_ids = HashSet::new();

        loop {
            match self.result_rx.try_recv() {
                Ok(response) => {
                    stats.merge(response.stats);
                    completed_request_ids.insert(response.request_id);
                    let is_current = self
                        .pending_by_character
                        .get(&response.character_id)
                        .is_some_and(|pending| pending.request_id == response.request_id);
                    if is_current {
                        self.pending_by_character.remove(&response.character_id);
                        self.completed_by_character
                            .insert(response.character_id, response);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    log::error!("Pathfinding worker result channel disconnected");
                    break;
                }
            }
        }

        self.pending_by_character
            .retain(|_, pending| !completed_request_ids.contains(&pending.request_id));
        stats
    }

    /// Shut down all pathfinding worker threads.
    pub fn shutdown(&mut self) {
        for sender in &self.job_senders {
            let _ = sender.send(WorkerMessage::Shutdown);
        }
        while let Some(handle) = self.handles.pop() {
            if let Err(error) = handle.join() {
                log::error!("Pathfinding worker thread panicked: {error:?}");
            }
        }
        self.pending_by_character.clear();
        self.completed_by_character.clear();
    }
}

impl Drop for PathfindingService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn worker_main(rx: Receiver<WorkerMessage>, result_tx: Sender<PathfindingResponse>) {
    let mut pathfinder = PathFinder::new();
    while let Ok(message) = rx.recv() {
        match message {
            WorkerMessage::Job(job) => {
                let direction = pathfinder.find_path_for_request(&job.request, &job.passability);
                let stats = pathfinder.take_interval_stats();
                let response = PathfindingResponse {
                    request_id: job.request_id,
                    character_id: job.character_id,
                    request: job.request,
                    direction,
                    stats,
                };
                if result_tx.send(response).is_err() {
                    break;
                }
            }
            WorkerMessage::Shutdown => break,
        }
    }
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            let trimmed = value.trim();
            !trimmed.is_empty()
                && trimmed != "0"
                && !trimmed.eq_ignore_ascii_case("false")
                && !trimmed.eq_ignore_ascii_case("off")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_finding::PathFindingTarget;

    #[test]
    fn spawn_rejects_zero_workers() {
        let result = std::panic::catch_unwind(|| PathfindingService::spawn(0));
        assert!(result.is_err());
    }

    #[test]
    fn service_drains_completed_worker_result() {
        let mut service = PathfindingService::spawn(1);
        let mut character = core::types::Character {
            x: 10,
            y: 10,
            dir: core::constants::DX_RIGHT,
            flags: core::constants::CharacterFlags::Player.bits(),
            ..core::types::Character::default()
        };
        character.data[78] = 0;
        let target = PathFindingTarget::new(12, 10, 0, 0, 0);
        let request = PathFindingRequest::from_character(&character, 1, target);
        let map_len = core::constants::SERVER_MAPX as usize * core::constants::SERVER_MAPY as usize;
        let map = vec![core::types::Map::default(); map_len];
        let items = vec![core::types::Item::default(); core::constants::MAXITEM];

        assert!(service.submit_if_absent(1, request, &map, &items));
        let mut stats = PathFindingStats::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            stats.merge(service.drain_completed());
            if service.completed_for(1).is_some() {
                break;
            }
            std::thread::yield_now();
        }

        let response = service.completed_for(1).expect("worker result");
        assert_eq!(response.character_id, 1);
        assert_eq!(response.direction, Some(core::constants::DX_RIGHT));
        assert_eq!(stats.calls, 1);
        service.shutdown();
    }

    #[test]
    fn duplicate_equivalent_request_is_not_enqueued() {
        let mut service = PathfindingService::spawn(1);
        let character = core::types::Character {
            x: 10,
            y: 10,
            dir: core::constants::DX_RIGHT,
            flags: core::constants::CharacterFlags::Player.bits(),
            ..core::types::Character::default()
        };
        let target = PathFindingTarget::new(12, 10, 0, 0, 0);
        let request = PathFindingRequest::from_character(&character, 1, target);
        let map_len = core::constants::SERVER_MAPX as usize * core::constants::SERVER_MAPY as usize;
        let map = vec![core::types::Map::default(); map_len];
        let items = vec![core::types::Item::default(); core::constants::MAXITEM];

        assert!(service.submit_if_absent(1, request, &map, &items));
        assert!(!service.submit_if_absent(1, request, &map, &items));
        service.shutdown();
    }
}
