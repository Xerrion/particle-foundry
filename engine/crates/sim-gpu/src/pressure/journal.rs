//! Opt-in host journal of existing pressure completion records and candidate outcomes.

use std::fmt::Write;

pub(crate) const MAX_DIAGNOSTIC_EVENTS: u32 = 65_536;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CandidateContext {
    pub epoch: u64,
    pub outer_tick: u64,
    pub candidate_id: u64,
    pub attempt: u32,
    pub state_revision: u64,
    pub accepted_time_before_s: f64,
    pub encoded_dt_s: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PressurePhase {
    Initialization,
    PcgBatch,
    FinalGate,
}

impl PressurePhase {
    fn name(self) -> &'static str {
        match self {
            Self::Initialization => "initialization",
            Self::PcgBatch => "pcgBatch",
            Self::FinalGate => "finalGate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CandidateOutcome {
    Accepted,
    NumericalRejection,
    TerminalRejection,
}

impl CandidateOutcome {
    fn name(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::NumericalRejection => "numericalRejection",
            Self::TerminalRejection => "terminalRejection",
        }
    }
}

#[derive(Debug)]
enum EventKind {
    CandidateBegin {
        timestamp_ms: f64,
    },
    PressureStatus {
        phase: PressurePhase,
        batch_index: u32,
        requested_iterations: u32,
        map_started_ms: f64,
        map_completed_ms: f64,
        status_words: [u32; 8],
    },
    CandidateOutcome {
        timestamp_ms: f64,
        outcome: CandidateOutcome,
    },
}

#[derive(Debug)]
struct Event {
    sequence: u64,
    context: CandidateContext,
    kind: EventKind,
}

pub(crate) struct PressureJournal {
    capacity: usize,
    events: Vec<Event>,
    dropped_events: u64,
    sequence: u64,
    candidate: Option<CandidateContext>,
    #[cfg(not(target_arch = "wasm32"))]
    clock: std::time::Instant,
    #[cfg(target_arch = "wasm32")]
    clock: web_sys::Performance,
}

fn clock_domain() -> &'static str {
    #[cfg(not(target_arch = "wasm32"))]
    return "native-instant";
    #[cfg(target_arch = "wasm32")]
    return "browser-performance-now";
}

impl PressureJournal {
    pub(crate) fn new(capacity: u32) -> Result<Self, String> {
        if !(1..=MAX_DIAGNOSTIC_EVENTS).contains(&capacity) {
            return Err(format!(
                "GPU pressure diagnostic capacity must be an integer between 1 and {MAX_DIAGNOSTIC_EVENTS}"
            ));
        }
        #[cfg(target_arch = "wasm32")]
        let clock = web_sys::window()
            .and_then(|window| window.performance())
            .ok_or("GPU pressure diagnostics need the browser performance clock")?;
        Ok(Self {
            capacity: capacity as usize,
            events: Vec::with_capacity(capacity as usize),
            dropped_events: 0,
            sequence: 0,
            candidate: None,
            #[cfg(not(target_arch = "wasm32"))]
            clock: std::time::Instant::now(),
            #[cfg(target_arch = "wasm32")]
            clock,
        })
    }

    pub(crate) fn now_ms(&self) -> f64 {
        #[cfg(not(target_arch = "wasm32"))]
        return self.clock.elapsed().as_secs_f64() * 1000.0;
        #[cfg(target_arch = "wasm32")]
        return self.clock.now();
    }

    fn push(&mut self, context: CandidateContext, kind: EventKind) {
        self.sequence = self.sequence.saturating_add(1);
        if self.events.len() == self.capacity {
            // Keep a coherent prefix, rather than overwriting its candidate beginnings.
            self.dropped_events = self.dropped_events.saturating_add(1);
            return;
        }
        self.events.push(Event {
            sequence: self.sequence,
            context,
            kind,
        });
    }

    pub(crate) fn begin_candidate(&mut self, context: CandidateContext) {
        self.candidate = Some(context);
        self.push(
            context,
            EventKind::CandidateBegin {
                timestamp_ms: self.now_ms(),
            },
        );
    }

    pub(crate) fn pressure_status(
        &mut self,
        phase: PressurePhase,
        batch_index: u32,
        requested_iterations: u32,
        map_started_ms: f64,
        map_completed_ms: f64,
        status_words: [u32; 8],
    ) {
        if let Some(context) = self.candidate {
            self.push(
                context,
                EventKind::PressureStatus {
                    phase,
                    batch_index,
                    requested_iterations,
                    map_started_ms,
                    map_completed_ms,
                    status_words,
                },
            );
        }
    }

    pub(crate) fn finish_candidate(&mut self, outcome: CandidateOutcome) {
        if let Some(context) = self.candidate.take() {
            self.push(
                context,
                EventKind::CandidateOutcome {
                    timestamp_ms: self.now_ms(),
                    outcome,
                },
            );
        }
    }

    pub(crate) fn empty_json() -> String {
        format!(
            "{{\"version\":1,\"clockDomain\":\"{}\",\"capacity\":0,\"droppedEvents\":0,\"events\":[]}}",
            clock_domain()
        )
    }

    pub(crate) fn take_json(&mut self) -> String {
        let mut json = format!(
            "{{\"version\":1,\"clockDomain\":\"{}\",\"capacity\":{},\"droppedEvents\":{},\"events\":[",
            clock_domain(),
            self.capacity,
            self.dropped_events
        );
        for (index, event) in self.events.drain(..).enumerate() {
            if index != 0 {
                json.push(',');
            }
            let context = event.context;
            write!(json,
                "{{\"sequence\":{},\"epoch\":{},\"outerTick\":{},\"candidateId\":{},\"attempt\":{},\"stateRevision\":{},\"acceptedTimeBeforeS\":{},\"encodedDtS\":{},",
                event.sequence, context.epoch, context.outer_tick, context.candidate_id,
                context.attempt, context.state_revision, context.accepted_time_before_s,
                context.encoded_dt_s,
            ).expect("writing to string");
            match event.kind {
                EventKind::CandidateBegin { timestamp_ms } => {
                    write!(
                        json,
                        "\"type\":\"candidateBegin\",\"timestampMs\":{timestamp_ms}"
                    )
                    .expect("writing to string");
                }
                EventKind::PressureStatus {
                    phase,
                    batch_index,
                    requested_iterations,
                    map_started_ms,
                    map_completed_ms,
                    status_words,
                } => {
                    write!(json,
                        "\"type\":\"pressureStatus\",\"phase\":\"{}\",\"batchIndex\":{batch_index},\"requestedIterations\":{requested_iterations},\"mapStartedMs\":{map_started_ms},\"mapCompletedMs\":{map_completed_ms},\"statusWords\":[",
                        phase.name(),
                    ).expect("writing to string");
                    for (index, word) in status_words.into_iter().enumerate() {
                        if index != 0 {
                            json.push(',');
                        }
                        write!(json, "{word}").expect("writing to string");
                    }
                    json.push(']');
                }
                EventKind::CandidateOutcome {
                    timestamp_ms,
                    outcome,
                } => {
                    write!(json, "\"type\":\"candidateOutcome\",\"timestampMs\":{timestamp_ms},\"outcome\":\"{}\"", outcome.name())
                        .expect("writing to string");
                }
            }
            json.push('}');
        }
        json.push_str("]}");
        self.dropped_events = 0;
        json
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(candidate_id: u64, attempt: u32) -> CandidateContext {
        CandidateContext {
            epoch: 3,
            outer_tick: 8,
            candidate_id,
            attempt,
            state_revision: 9,
            accepted_time_before_s: 0.125,
            encoded_dt_s: 0.01,
        }
    }

    #[test]
    fn bounded_prefix_reports_overflow_and_drain_keeps_sequence() {
        assert!(PressureJournal::new(0).is_err());
        assert!(PressureJournal::new(MAX_DIAGNOSTIC_EVENTS + 1).is_err());
        let mut journal = PressureJournal::new(2).unwrap();
        journal.begin_candidate(context(1, 1));
        journal.pressure_status(PressurePhase::Initialization, 0, 0, 1.0, 2.0, [0; 8]);
        journal.finish_candidate(CandidateOutcome::NumericalRejection);
        journal.begin_candidate(context(2, 2));
        journal.finish_candidate(CandidateOutcome::Accepted);
        let json = journal.take_json();
        assert!(json.contains("\"droppedEvents\":3"));
        assert!(json.contains("\"sequence\":1"));
        assert!(json.contains("\"sequence\":2"));
        assert!(!json.contains("\"candidateId\":2"));
        assert!(!json.contains("candidateOutcome"));
        journal.begin_candidate(context(3, 1));
        journal.finish_candidate(CandidateOutcome::TerminalRejection);
        let next = journal.take_json();
        assert!(next.contains("\"droppedEvents\":0"));
        assert!(next.contains("\"sequence\":6"));
        assert!(next.contains("\"sequence\":7"));
        assert!(next.contains("terminalRejection"));
        assert!(journal.take_json().ends_with("\"events\":[]}"));
    }

    #[test]
    fn raw_nonfinite_status_bits_preserve_initial_batch_and_failed_gate() {
        let mut journal = PressureJournal::new(8).unwrap();
        journal.begin_candidate(context(4, 3));
        let words = [
            1,
            2,
            3,
            f32::NAN.to_bits(),
            f32::INFINITY.to_bits(),
            16,
            0,
            1,
        ];
        for (phase, index, iterations) in [
            (PressurePhase::Initialization, 0, 0),
            (PressurePhase::PcgBatch, 1, 8),
            (PressurePhase::FinalGate, 2, 0),
        ] {
            journal.pressure_status(phase, index, iterations, 2.0, 3.0, words);
        }
        journal.finish_candidate(CandidateOutcome::NumericalRejection);
        let json = journal.take_json();
        assert_eq!(
            json.matches("\"statusWords\":[1,2,3,2143289344,2139095040,16,0,1]")
                .count(),
            3
        );
        assert!(json.contains("\"attempt\":3"));
        assert!(json.contains("\"phase\":\"initialization\""));
        assert!(json.contains("\"phase\":\"pcgBatch\",\"batchIndex\":1,\"requestedIterations\":8"));
        assert!(json.contains("\"phase\":\"finalGate\""));
        assert!(json.contains("\"outcome\":\"numericalRejection\""));
        assert!(!json.contains("NaN"));
        assert!(!json.contains("inf"));
    }

    #[test]
    fn pre_pressure_rejection_still_has_begin_and_outcome() {
        let mut journal = PressureJournal::new(8).unwrap();
        journal.begin_candidate(context(1, 1));
        journal.finish_candidate(CandidateOutcome::TerminalRejection);
        let json = journal.take_json();
        assert!(json.contains("candidateBegin"));
        assert!(json.contains("terminalRejection"));
        assert!(!json.contains("pressureStatus"));
        assert_eq!(json.matches("\"candidateId\":1").count(), 2);
    }
}
