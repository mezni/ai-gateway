use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, AtomicUsize, Ordering},
};

use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GatewayPhase {
    Initializing = 0,
    Ready = 1,
    ShuttingDown = 2,
    Stopped = 3,
}

impl GatewayPhase {
    fn code(self) -> u8 {
        self as u8
    }

    fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Initializing,
            1 => Self::Ready,
            2 => Self::ShuttingDown,
            3 => Self::Stopped,
            _ => Self::Stopped,
        }
    }

    fn rank(self) -> u8 {
        self.code()
    }
}

#[derive(Debug)]
pub struct GatewayLifecycle {
    phase: AtomicU8,
    in_flight: Arc<AtomicUsize>,
    in_flight_updates: watch::Sender<usize>,
    admission_lock: Mutex<()>,
}

impl GatewayLifecycle {
    pub fn new() -> Self {
        let (in_flight_updates, _) = watch::channel(0);
        Self {
            phase: AtomicU8::new(GatewayPhase::Initializing.code()),
            in_flight: Arc::new(AtomicUsize::new(0)),
            in_flight_updates,
            admission_lock: Mutex::new(()),
        }
    }

    pub fn phase(&self) -> GatewayPhase {
        GatewayPhase::from_code(self.phase.load(Ordering::Acquire))
    }

    pub fn is_ready(&self) -> bool {
        self.phase() == GatewayPhase::Ready
    }

    pub fn mark_ready(&self) {
        self.transition_to(GatewayPhase::Ready);
    }

    pub fn begin_shutdown(&self) {
        self.transition_to(GatewayPhase::ShuttingDown);
    }

    pub fn mark_stopped(&self) {
        self.transition_to(GatewayPhase::Stopped);
    }

    pub fn try_admit_chat(&self) -> Option<ChatAdmissionGuard> {
        let _admission = self
            .admission_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !self.is_ready() {
            return None;
        }
        let in_flight = self.increment_in_flight()?;
        self.in_flight_updates.send_replace(in_flight);
        Some(ChatAdmissionGuard {
            in_flight: Arc::clone(&self.in_flight),
            in_flight_updates: self.in_flight_updates.clone(),
        })
    }

    pub async fn wait_for_zero(&self) {
        let mut updates = self.in_flight_updates.subscribe();
        loop {
            if self.in_flight.load(Ordering::Acquire) == 0 && *updates.borrow() == 0 {
                return;
            }
            if updates.changed().await.is_err() {
                return;
            }
        }
    }

    fn transition_to(&self, target: GatewayPhase) {
        let _admission = self
            .admission_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut current = self.phase();
        loop {
            if target.rank() <= current.rank() {
                return;
            }
            match self.phase.compare_exchange_weak(
                current.code(),
                target.code(),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(observed) => current = GatewayPhase::from_code(observed),
            }
        }
    }

    fn increment_in_flight(&self) -> Option<usize> {
        let mut current = self.in_flight.load(Ordering::Acquire);
        loop {
            let next = current.checked_add(1)?;
            match self.in_flight.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(next),
                Err(observed) => current = observed,
            }
        }
    }
}

fn decrement_in_flight(in_flight: &AtomicUsize, updates: &watch::Sender<usize>) {
    let mut current = in_flight.load(Ordering::Acquire);
    loop {
        let Some(next) = current.checked_sub(1) else {
            return;
        };
        match in_flight.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => {
                updates.send_replace(next);
                return;
            }
            Err(observed) => current = observed,
        }
    }
}

impl Default for GatewayLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ChatAdmissionGuard {
    in_flight: Arc<AtomicUsize>,
    in_flight_updates: watch::Sender<usize>,
}

impl Drop for ChatAdmissionGuard {
    fn drop(&mut self) {
        decrement_in_flight(&self.in_flight, &self.in_flight_updates);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::Ordering::{Acquire, SeqCst},
        },
        time::Duration,
    };

    use super::*;

    #[test]
    fn new_starts_in_initializing_phase() {
        let lifecycle = GatewayLifecycle::new();

        assert_eq!(lifecycle.phase(), GatewayPhase::Initializing);
    }

    #[test]
    fn new_starts_not_ready() {
        let lifecycle = GatewayLifecycle::new();

        assert!(!lifecycle.is_ready());
    }

    #[test]
    fn mark_ready_moves_initializing_to_ready() {
        let lifecycle = GatewayLifecycle::new();

        lifecycle.mark_ready();

        assert_eq!(lifecycle.phase(), GatewayPhase::Ready);
        assert!(lifecycle.is_ready());
    }

    #[test]
    fn begin_shutdown_moves_ready_to_shutting_down() {
        let lifecycle = GatewayLifecycle::new();
        lifecycle.mark_ready();

        lifecycle.begin_shutdown();

        assert_eq!(lifecycle.phase(), GatewayPhase::ShuttingDown);
    }

    #[test]
    fn mark_stopped_moves_shutting_down_to_stopped() {
        let lifecycle = GatewayLifecycle::new();
        lifecycle.begin_shutdown();

        lifecycle.mark_stopped();

        assert_eq!(lifecycle.phase(), GatewayPhase::Stopped);
    }

    #[test]
    fn mark_ready_does_not_move_backward_after_shutdown() {
        let lifecycle = GatewayLifecycle::new();
        lifecycle.begin_shutdown();

        lifecycle.mark_ready();

        assert_eq!(lifecycle.phase(), GatewayPhase::ShuttingDown);
    }

    #[test]
    fn begin_shutdown_does_not_move_backward_after_stopped() {
        let lifecycle = GatewayLifecycle::new();
        lifecycle.begin_shutdown();
        lifecycle.mark_stopped();

        lifecycle.begin_shutdown();

        assert_eq!(lifecycle.phase(), GatewayPhase::Stopped);
    }

    #[test]
    fn repeated_shutdown_is_idempotent() {
        let lifecycle = GatewayLifecycle::new();

        lifecycle.begin_shutdown();
        lifecycle.begin_shutdown();

        assert_eq!(lifecycle.phase(), GatewayPhase::ShuttingDown);
    }

    #[test]
    fn try_admit_chat_rejects_while_initializing() {
        let lifecycle = GatewayLifecycle::new();

        assert!(lifecycle.try_admit_chat().is_none());
    }

    #[test]
    fn try_admit_chat_accepts_while_ready() {
        let lifecycle = GatewayLifecycle::new();
        lifecycle.mark_ready();

        assert!(lifecycle.try_admit_chat().is_some());
    }

    #[test]
    fn try_admit_chat_rejects_after_shutdown_begins() {
        let lifecycle = GatewayLifecycle::new();
        lifecycle.mark_ready();
        lifecycle.begin_shutdown();

        assert!(lifecycle.try_admit_chat().is_none());
    }

    #[test]
    fn try_admit_chat_rejects_after_stopping() {
        let lifecycle = GatewayLifecycle::new();
        lifecycle.mark_ready();
        lifecycle.begin_shutdown();
        lifecycle.mark_stopped();

        assert!(lifecycle.try_admit_chat().is_none());
    }

    #[test]
    fn dropping_admission_guard_decrements_in_flight_count() {
        let lifecycle = Arc::new(GatewayLifecycle::new());
        lifecycle.mark_ready();
        let guard = lifecycle.try_admit_chat().unwrap();

        assert_eq!(lifecycle.in_flight.load(Acquire), 1);
        drop(guard);
        assert_eq!(lifecycle.in_flight.load(SeqCst), 0);
    }

    #[tokio::test]
    async fn aborting_admitted_task_drops_guard_and_releases_in_flight_count() {
        let lifecycle = Arc::new(GatewayLifecycle::new());
        lifecycle.mark_ready();
        let task_lifecycle = Arc::clone(&lifecycle);
        let task = tokio::spawn(async move {
            let _guard = task_lifecycle.try_admit_chat().unwrap();
            std::future::pending::<()>().await;
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            while lifecycle.in_flight.load(Acquire) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        task.abort();
        let result = task.await;

        assert!(result.unwrap_err().is_cancelled());
        tokio::time::timeout(Duration::from_secs(1), lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn wait_for_zero_returns_immediately_when_no_requests_are_active() {
        let lifecycle = GatewayLifecycle::new();

        tokio::time::timeout(Duration::from_secs(1), lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn wait_for_zero_is_notified_when_admitted_guard_drops() {
        let lifecycle = Arc::new(GatewayLifecycle::new());
        lifecycle.mark_ready();
        let guard = lifecycle.try_admit_chat().unwrap();
        let waiting_lifecycle = Arc::clone(&lifecycle);
        let waiter = tokio::spawn(async move {
            waiting_lifecycle.wait_for_zero().await;
        });
        tokio::task::yield_now().await;

        assert!(!waiter.is_finished());
        drop(guard);
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .unwrap()
            .unwrap();
    }
}
