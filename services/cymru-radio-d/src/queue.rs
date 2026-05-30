//! TDM transmit queue with strict priority + FIFO within a priority class.
//! E-stop (class 0) preempts everything (RFC 0003 §5). Mirrors safety_class.

use crate::frame::Frame;

/// Transmit priority classes (lower value = higher priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// e-stop / critical — preempts the queue.
    EStop = 0,
    /// control / caution.
    Control = 1,
    /// normal data / routine.
    Data = 2,
    /// background (telemetry, position).
    Background = 3,
}

#[derive(Debug)]
struct Item {
    prio: Priority,
    seq: u64,
    frame: Frame,
}

/// Strict-priority FIFO transmit queue.
#[derive(Debug, Default)]
pub struct TxQueue {
    items: Vec<Item>,
    seq: u64,
}

impl TxQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Enqueue a frame at the given priority.
    pub fn push(&mut self, prio: Priority, frame: Frame) {
        self.items.push(Item { prio, seq: self.seq, frame });
        self.seq += 1;
    }

    /// Pop the next frame to transmit: highest priority, then FIFO.
    pub fn pop(&mut self) -> Option<Frame> {
        if self.items.is_empty() {
            return None;
        }
        // Find the best (min prio, then min seq).
        let mut best = 0usize;
        for i in 1..self.items.len() {
            let a = &self.items[i];
            let b = &self.items[best];
            if a.prio < b.prio || (a.prio == b.prio && a.seq < b.seq) {
                best = i;
            }
        }
        Some(self.items.remove(best).frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Frame, BROADCAST};

    fn f(tag: u8) -> Frame {
        Frame::new(0x04, [tag; 16], BROADCAST, [0; 4], vec![tag])
    }

    #[test]
    fn estop_preempts() {
        let mut q = TxQueue::new();
        q.push(Priority::Data, f(1));
        q.push(Priority::Background, f(2));
        q.push(Priority::EStop, f(3)); // arrives last, must leave first
        assert_eq!(q.pop().unwrap().sender[0], 3); // e-stop first
        assert_eq!(q.pop().unwrap().sender[0], 1); // then data (higher than background)
        assert_eq!(q.pop().unwrap().sender[0], 2);
        assert!(q.pop().is_none());
    }

    #[test]
    fn fifo_within_priority() {
        let mut q = TxQueue::new();
        q.push(Priority::Data, f(10));
        q.push(Priority::Data, f(11));
        assert_eq!(q.pop().unwrap().sender[0], 10);
        assert_eq!(q.pop().unwrap().sender[0], 11);
    }
}
