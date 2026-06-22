use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Engine state, mirroring the original AuroraBox state machine
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state")]
pub enum EngineState {
    #[serde(rename = "idle")]
    Idle {
        epoch: u64,
    },
    #[serde(rename = "starting")]
    Starting {
        since: i64,
        epoch: u64,
        mode: String,
    },
    #[serde(rename = "running")]
    Running {
        since: i64,
        epoch: u64,
        mode: String,
    },
    #[serde(rename = "stopping")]
    Stopping {
        since: i64,
        epoch: u64,
    },
    #[serde(rename = "failed")]
    Failed {
        reason: String,
        at: i64,
        epoch: u64,
    },
}

impl EngineState {
    pub fn is_running(&self) -> bool {
        matches!(self, EngineState::Running { .. })
    }
}

impl Default for EngineState {
    fn default() -> Self {
        EngineState::Idle { epoch: 0 }
    }
}

/// A state transition intent
#[derive(Debug)]
pub enum Intent {
    Start { mode: String },
    MarkRunning,
    Stop,
    MarkIdle,
    Fail { reason: String },
    ClearFailure,
}

struct StateManager {
    state: Mutex<EngineState>,
    epoch: AtomicU64,
}

lazy_static::lazy_static! {
    static ref STATE: StateManager = StateManager::new();
}

impl StateManager {
    fn new() -> Self {
        StateManager {
            state: Mutex::new(EngineState::default()),
            epoch: AtomicU64::new(0),
        }
    }

    fn next_epoch(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::SeqCst)
    }
}

/// Transition the engine state
pub fn transition(intent: Intent) -> anyhow::Result<EngineState> {
    STATE.transition(intent)
}

impl StateManager {
    fn transition(&self, intent: Intent) -> anyhow::Result<EngineState> {
        let mut state = self.state.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        let epoch = self.next_epoch();
        let new_state: EngineState;

        match intent {
            Intent::Start { mode } => match &*state {
                EngineState::Idle { .. } | EngineState::Failed { .. } => {
                    new_state = EngineState::Starting {
                        since: now,
                        epoch,
                        mode,
                    };
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Cannot start engine from state: {:?}",
                        *state
                    ));
                }
            },

            Intent::MarkRunning => match &*state {
                EngineState::Starting { mode, .. } => {
                    new_state = EngineState::Running {
                        since: now,
                        epoch,
                        mode: mode.clone(),
                    };
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Cannot mark running from state: {:?}",
                        *state
                    ));
                }
            },

            Intent::Stop => match &*state {
                EngineState::Running { .. } | EngineState::Starting { .. } => {
                    new_state = EngineState::Stopping { since: now, epoch };
                }
                _ => {
                    log::warn!("Engine not running, skipping stop");
                    return Ok(state.clone());
                }
            },

            Intent::MarkIdle => {
                new_state = EngineState::Idle { epoch };
            }

            Intent::Fail { reason } => match &*state {
                EngineState::Starting { .. } | EngineState::Running { .. } => {
                    new_state = EngineState::Failed {
                        reason,
                        at: now,
                        epoch,
                    };
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Cannot fail engine from state: {:?}",
                        *state
                    ));
                }
            },

            Intent::ClearFailure => match &*state {
                EngineState::Failed { .. } => {
                    new_state = EngineState::Idle { epoch };
                }
                _ => {
                    return Ok(state.clone());
                }
            },
        }

        log::info!("Engine state transition: {:?} -> {:?}", *state, new_state);
        *state = new_state.clone();
        Ok(new_state)
    }
}

/// Get the current engine state snapshot
pub fn snapshot() -> EngineState {
    let state = STATE.state.lock().unwrap();
    state.clone()
}

/// Check if the engine is in Running state
pub fn is_running() -> bool {
    snapshot().is_running()
}
