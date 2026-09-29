//! Bounded physical-time selection for detached CPU fluid candidates.
//!
//! This first selector covers one liquid, fixed walls, advection and explicit
//! viscosity. Phase sources, moving obstacles and reservoir flow need their own
//! stability bounds before those stages can use this clock.

use particle_sim::Grid;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::fluid::FaceValues;

/// Maximum multidimensional donor or receiver displacement per cell width.
pub const MAX_CELL_CFL: f64 = 0.5;

static NEXT_CLOCK_ID: AtomicU64 = AtomicU64::new(1);

/// Invalid stability input or attempt to accept a stale candidate.
#[derive(Clone, Debug, PartialEq)]
pub enum SubstepError {
    /// A duration, cell width or viscosity bound is outside its valid range.
    InvalidScalar(&'static str),
    /// A face array does not match the supplied grid.
    Length {
        /// Name of the rejected array.
        field: &'static str,
        /// Required number of entries.
        expected: usize,
        /// Supplied number of entries.
        actual: usize,
    },
    /// A MAC face speed is NaN or infinity.
    NonFiniteSpeed {
        /// Horizontal `u` or vertical `v` face array.
        axis: &'static str,
        /// Row-major face index.
        index: usize,
    },
    /// Finite inputs cannot produce a positive representable physical-time step.
    UnrepresentableStep,
    /// A candidate was selected from another accepted-time or budget state.
    StaleCandidate,
    /// No further unique clock identities can be allocated in this process.
    ClockIdentityExhausted,
}

impl std::fmt::Display for SubstepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SubstepError {}

/// Why physical time cannot advance within the current work budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseReason {
    /// The outer interval still has time left after all allowed substeps.
    SubstepBudgetExhausted,
}

/// A proposed duration that does not mutate fluid state or accepted time.
#[derive(Debug, PartialEq)]
pub struct SubstepCandidate {
    clock_id: u64,
    dt_s: f64,
    max_cfl_dt_s: Option<f64>,
    max_diffusion_dt_s: Option<f64>,
    start_time_s: f64,
    start_substeps: u32,
    outer_duration_s: f64,
    max_substeps: u32,
}

impl SubstepCandidate {
    /// Finite, positive duration for one detached fluid candidate.
    pub fn dt_s(&self) -> f64 {
        self.dt_s
    }

    /// Maximum duration allowed by the cellwise multidimensional CFL bound.
    /// `None` means zero speed or no finite CFL limit at this scale.
    pub fn max_cfl_dt_s(&self) -> Option<f64> {
        self.max_cfl_dt_s
    }

    /// Maximum duration allowed by explicit two-dimensional viscosity.
    /// `None` means viscosity is zero or imposes no finite limit.
    pub fn max_diffusion_dt_s(&self) -> Option<f64> {
        self.max_diffusion_dt_s
    }
}

/// Outcome of a selection attempt within one outer interval.
#[derive(Debug, PartialEq)]
pub enum SubstepSelection {
    /// Run downstream stages against detached buffers, then accept on success.
    Candidate(SubstepCandidate),
    /// The full requested outer duration was accepted.
    Complete,
    /// Preserve the unadvanced time and report the work limit to the caller.
    Paused {
        /// The limit that stopped physical progress.
        reason: PauseReason,
        /// Outer duration that has not yet been accepted.
        remaining_s: f64,
    },
}

/// Accepted physical time and work count for one requested outer interval.
///
/// The clock owns no mass, velocity or pressure arrays. Selecting a candidate
/// leaves this clock unchanged. A caller accepts it only after all detached
/// fluid stages succeed, so a rejected stage cannot advance physical time.
#[derive(Debug, PartialEq)]
pub struct SubstepClock {
    clock_id: u64,
    outer_duration_s: f64,
    accepted_time_s: f64,
    accepted_substeps: u32,
    max_substeps: u32,
}

impl SubstepClock {
    /// Starts one outer interval with no accepted physical time.
    pub fn new(outer_duration_s: f64, max_substeps: u32) -> Result<Self, SubstepError> {
        if !outer_duration_s.is_finite() || outer_duration_s <= 0.0 {
            return Err(SubstepError::InvalidScalar("outer_duration_s"));
        }
        if max_substeps == 0 {
            return Err(SubstepError::InvalidScalar("max_substeps"));
        }
        let clock_id = NEXT_CLOCK_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .map_err(|_| SubstepError::ClockIdentityExhausted)?;
        Ok(Self {
            clock_id,
            outer_duration_s,
            accepted_time_s: 0.0,
            accepted_substeps: 0,
            max_substeps,
        })
    }

    /// Requested duration for this outer interval.
    pub fn outer_duration_s(&self) -> f64 {
        self.outer_duration_s
    }

    /// Physical time committed after successful substeps only.
    pub fn accepted_time_s(&self) -> f64 {
        self.accepted_time_s
    }

    /// Physical time still to be advanced within this outer interval.
    pub fn remaining_time_s(&self) -> f64 {
        self.outer_duration_s - self.accepted_time_s
    }

    /// Number of accepted substeps, excluding failed candidates.
    pub fn accepted_substeps(&self) -> u32 {
        self.accepted_substeps
    }

    /// Selects a finite positive duration without changing this clock.
    ///
    /// The CFL rate is the larger signed incoming or outgoing face-speed sum
    /// for each cell. Uniform flow crosses a cell once, while diverging flow
    /// across two faces counts both exits. The maximum cell rate bounds the
    /// geometric donor displacement by [`MAX_CELL_CFL`]. The explicit 2D
    /// diffusion limit is `dx²/(4 nu)`, matching the fixed-wall shear reference.
    /// Zero viscosity has no diffusion limit. Completed or exhausted clocks
    /// return their state without inspecting the velocity inputs.
    pub fn select(
        &self,
        grid: Grid,
        velocity_m_s: &FaceValues,
        max_kinematic_viscosity_m2_s: f64,
    ) -> Result<SubstepSelection, SubstepError> {
        let remaining_s = self.remaining_time_s();
        if remaining_s == 0.0 {
            return Ok(SubstepSelection::Complete);
        }
        if self.accepted_substeps == self.max_substeps {
            return Ok(SubstepSelection::Paused {
                reason: PauseReason::SubstepBudgetExhausted,
                remaining_s,
            });
        }

        let cell_width_m = grid.cell_width_m();
        if !max_kinematic_viscosity_m2_s.is_finite() || max_kinematic_viscosity_m2_s < 0.0 {
            return Err(SubstepError::InvalidScalar("max_kinematic_viscosity_m2_s"));
        }
        check_faces("u", &velocity_m_s.u, grid.u_faces())?;
        check_faces("v", &velocity_m_s.v, grid.v_faces())?;

        let max_rate_m_s = max_cell_face_rate(grid, velocity_m_s)?;
        let max_cfl_dt_s = if max_rate_m_s == 0.0 {
            None
        } else {
            let limit = MAX_CELL_CFL * cell_width_m / max_rate_m_s;
            if limit.is_infinite() {
                None
            } else if !limit.is_finite() || limit <= 0.0 {
                return Err(SubstepError::UnrepresentableStep);
            } else {
                Some(limit)
            }
        };
        let max_diffusion_dt_s = diffusion_limit(cell_width_m, max_kinematic_viscosity_m2_s)?;
        let mut dt_s = remaining_s;
        if let Some(limit) = max_cfl_dt_s {
            dt_s = dt_s.min(limit);
        }
        if let Some(limit) = max_diffusion_dt_s {
            dt_s = dt_s.min(limit);
        }
        if !dt_s.is_finite() || dt_s <= 0.0 || self.accepted_time_s + dt_s <= self.accepted_time_s {
            return Err(SubstepError::UnrepresentableStep);
        }

        Ok(SubstepSelection::Candidate(SubstepCandidate {
            clock_id: self.clock_id,
            dt_s,
            max_cfl_dt_s,
            max_diffusion_dt_s,
            start_time_s: self.accepted_time_s,
            start_substeps: self.accepted_substeps,
            outer_duration_s: self.outer_duration_s,
            max_substeps: self.max_substeps,
        }))
    }

    /// Commits accepted physical time after every detached fluid stage succeeds.
    ///
    /// A replayed or stale candidate is rejected without changing the clock.
    /// The fluid owner must separately commit its validated stage buffers.
    /// If face speeds or viscosity change before acceptance, select again.
    pub fn accept(&mut self, candidate: SubstepCandidate) -> Result<(), SubstepError> {
        if candidate.clock_id != self.clock_id
            || candidate.start_time_s != self.accepted_time_s
            || candidate.start_substeps != self.accepted_substeps
            || candidate.outer_duration_s != self.outer_duration_s
            || candidate.max_substeps != self.max_substeps
            || self.accepted_substeps >= self.max_substeps
        {
            return Err(SubstepError::StaleCandidate);
        }
        let remaining_s = self.remaining_time_s();
        if !candidate.dt_s.is_finite() || candidate.dt_s <= 0.0 || candidate.dt_s > remaining_s {
            return Err(SubstepError::StaleCandidate);
        }
        let next_time_s = if candidate.dt_s == remaining_s {
            self.outer_duration_s
        } else {
            self.accepted_time_s + candidate.dt_s
        };
        if !next_time_s.is_finite() || next_time_s <= self.accepted_time_s {
            return Err(SubstepError::UnrepresentableStep);
        }
        self.accepted_time_s = next_time_s;
        self.accepted_substeps += 1;
        Ok(())
    }
}

fn diffusion_limit(
    cell_width_m: f64,
    kinematic_viscosity_m2_s: f64,
) -> Result<Option<f64>, SubstepError> {
    if kinematic_viscosity_m2_s == 0.0 {
        return Ok(None);
    }
    let quarter_squared_width = cell_width_m * cell_width_m * 0.25;
    let limit = if quarter_squared_width.is_finite() && quarter_squared_width > 0.0 {
        quarter_squared_width / kinematic_viscosity_m2_s
    } else {
        (cell_width_m / kinematic_viscosity_m2_s) * (cell_width_m * 0.25)
    };
    if limit.is_infinite() {
        Ok(None)
    } else if !limit.is_finite() || limit <= 0.0 {
        Err(SubstepError::UnrepresentableStep)
    } else {
        Ok(Some(limit))
    }
}

fn check_faces(axis: &'static str, faces: &[f64], expected: usize) -> Result<(), SubstepError> {
    if faces.len() != expected {
        return Err(SubstepError::Length {
            field: axis,
            expected,
            actual: faces.len(),
        });
    }
    for (index, speed) in faces.iter().enumerate() {
        if !speed.is_finite() {
            return Err(SubstepError::NonFiniteSpeed { axis, index });
        }
    }
    Ok(())
}

fn max_cell_face_rate(grid: Grid, faces: &FaceValues) -> Result<f64, SubstepError> {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let mut max_rate_m_s = 0.0_f64;
    for y in 0..height {
        for x in 0..width {
            let u_left = faces.u[y * (width + 1) + x];
            let u_right = faces.u[y * (width + 1) + x + 1];
            let v_top = faces.v[y * width + x];
            let v_bottom = faces.v[(y + 1) * width + x];
            let incoming =
                u_left.max(0.0) + (-u_right).max(0.0) + v_top.max(0.0) + (-v_bottom).max(0.0);
            let outgoing =
                (-u_left).max(0.0) + u_right.max(0.0) + (-v_top).max(0.0) + v_bottom.max(0.0);
            let rate = incoming.max(outgoing);
            if !rate.is_finite() {
                return Err(SubstepError::UnrepresentableStep);
            }
            max_rate_m_s = max_rate_m_s.max(rate);
        }
    }
    Ok(max_rate_m_s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use particle_sim::OUTER_DT_S;

    fn single_cell() -> Grid {
        Grid::new(1.0, 1.0).unwrap()
    }

    fn faces(u_left: f64, u_right: f64, v_top: f64, v_bottom: f64) -> FaceValues {
        FaceValues {
            u: vec![u_left, u_right],
            v: vec![v_top, v_bottom],
        }
    }

    fn candidate(selection: SubstepSelection) -> SubstepCandidate {
        match selection {
            SubstepSelection::Candidate(candidate) => candidate,
            other => panic!("expected candidate, got {other:?}"),
        }
    }

    #[test]
    fn three_metres_per_second_needs_ten_equal_one_dimensional_steps() {
        let velocity = faces(3.0, 3.0, 0.0, 0.0);
        let mut clock = SubstepClock::new(OUTER_DT_S, 12).unwrap();
        let cap_s = 0.005 / 3.0;
        let mut durations = Vec::new();
        loop {
            match clock.select(single_cell(), &velocity, 0.0).unwrap() {
                SubstepSelection::Candidate(step) => {
                    durations.push(step.dt_s());
                    assert!(step.dt_s() <= cap_s);
                    assert_eq!(step.max_cfl_dt_s(), Some(cap_s));
                    clock.accept(step).unwrap();
                }
                SubstepSelection::Complete => break,
                other => panic!("unexpected outcome {other:?}"),
            }
        }
        assert!(durations.len() >= 10);
        assert!(durations.iter().all(|dt| (dt - cap_s).abs() < 1e-15));
        assert_eq!(clock.accepted_time_s(), OUTER_DT_S);
    }

    #[test]
    fn opposing_or_multidimensional_faces_tighten_the_bound() {
        let clock = SubstepClock::new(OUTER_DT_S, 20).unwrap();
        let outward = candidate(
            clock
                .select(single_cell(), &faces(-3.0, 3.0, 0.0, 0.0), 0.0)
                .unwrap(),
        );
        assert_eq!(outward.max_cfl_dt_s(), Some(0.005 / 6.0));
        let diagonal = candidate(
            clock
                .select(single_cell(), &faces(3.0, 3.0, 4.0, 4.0), 0.0)
                .unwrap(),
        );
        assert_eq!(diagonal.max_cfl_dt_s(), Some(0.005 / 7.0));
    }

    #[test]
    fn viscosity_and_remaining_time_limit_candidate() {
        let mut clock = SubstepClock::new(0.002, 3).unwrap();
        let velocity = faces(0.0, 0.0, 0.0, 0.0);
        let first = candidate(clock.select(single_cell(), &velocity, 0.05).unwrap());
        assert_eq!(first.max_cfl_dt_s(), None);
        assert_eq!(first.max_diffusion_dt_s(), Some(0.0005));
        assert_eq!(first.dt_s(), 0.0005);
        clock.accept(first).unwrap();
        assert_eq!(clock.accepted_time_s(), 0.0005);
        assert_eq!(clock.remaining_time_s(), 0.0015);
    }

    #[test]
    fn diffusion_limit_survives_square_overflow() {
        let limit_s = diffusion_limit(1e155, 1e300).unwrap().unwrap();
        assert!((limit_s / 2.5e9 - 1.0).abs() < 1e-12);
    }

    #[test]
    fn refined_grid_uses_its_own_cell_width_for_cfl_and_diffusion() {
        let grid = Grid::with_cell_width(1.0, 1.0, 0.005).unwrap();
        let clock = SubstepClock::new(0.01, 20).unwrap();
        let step = candidate(
            clock
                .select(grid, &faces(3.0, 3.0, 0.0, 0.0), 0.05)
                .unwrap(),
        );
        assert_eq!(step.max_cfl_dt_s(), Some(0.0025 / 3.0));
        assert_eq!(step.max_diffusion_dt_s(), Some(0.000125));
        assert_eq!(step.dt_s(), 0.000125);
    }

    #[test]
    fn budget_pauses_with_unadvanced_time_instead_of_enlarging_dt() {
        let mut clock = SubstepClock::new(OUTER_DT_S, 2).unwrap();
        let velocity = faces(3.0, 3.0, 0.0, 0.0);
        for _ in 0..2 {
            let step = candidate(clock.select(single_cell(), &velocity, 0.0).unwrap());
            assert!(step.dt_s() <= 0.005 / 3.0);
            clock.accept(step).unwrap();
        }
        let remaining_s = clock.remaining_time_s();
        assert_eq!(
            clock.select(single_cell(), &velocity, 0.0).unwrap(),
            SubstepSelection::Paused {
                reason: PauseReason::SubstepBudgetExhausted,
                remaining_s,
            }
        );
        assert!(remaining_s > 0.0);
    }

    #[test]
    fn failed_stage_does_not_advance_and_stale_candidate_is_rejected() {
        let mut clock = SubstepClock::new(OUTER_DT_S, 3).unwrap();
        let velocity = faces(3.0, 3.0, 0.0, 0.0);
        let abandoned = candidate(clock.select(single_cell(), &velocity, 0.0).unwrap());
        assert_eq!(clock.accepted_time_s(), 0.0);
        assert_eq!(clock.accepted_substeps(), 0);
        let accepted = candidate(clock.select(single_cell(), &velocity, 0.0).unwrap());
        clock.accept(accepted).unwrap();
        let accepted_time_s = clock.accepted_time_s();
        assert_eq!(clock.accept(abandoned), Err(SubstepError::StaleCandidate));
        assert_eq!(clock.accepted_time_s(), accepted_time_s);
        assert_eq!(clock.accepted_substeps(), 1);
    }

    #[test]
    fn candidate_from_another_clock_is_rejected_even_with_equal_progress() {
        let first_clock = SubstepClock::new(OUTER_DT_S, 3).unwrap();
        let mut second_clock = SubstepClock::new(OUTER_DT_S, 3).unwrap();
        let first_velocity = faces(3.0, 3.0, 0.0, 0.0);
        let first_candidate = candidate(
            first_clock
                .select(single_cell(), &first_velocity, 0.0)
                .unwrap(),
        );
        let second_velocity = faces(0.0, 0.0, 0.0, 0.0);
        assert_eq!(
            second_clock.accept(first_candidate),
            Err(SubstepError::StaleCandidate)
        );
        assert_eq!(second_clock.accepted_time_s(), 0.0);
        assert_eq!(second_clock.accepted_substeps(), 0);
        second_clock
            .accept(candidate(
                second_clock
                    .select(single_cell(), &second_velocity, 0.0)
                    .unwrap(),
            ))
            .unwrap();
        assert_eq!(second_clock.accepted_time_s(), OUTER_DT_S);
    }

    #[test]
    fn invalid_inputs_are_rejected_before_a_candidate_is_selected() {
        assert_eq!(
            SubstepClock::new(0.0, 1),
            Err(SubstepError::InvalidScalar("outer_duration_s"))
        );
        assert_eq!(
            SubstepClock::new(1.0, 0),
            Err(SubstepError::InvalidScalar("max_substeps"))
        );
        let clock = SubstepClock::new(OUTER_DT_S, 1).unwrap();
        assert_eq!(
            clock.select(single_cell(), &faces(0.0, 0.0, 0.0, 0.0), -1.0),
            Err(SubstepError::InvalidScalar("max_kinematic_viscosity_m2_s"))
        );
        assert_eq!(
            clock.select(single_cell(), &faces(f64::NAN, 0.0, 0.0, 0.0), 0.0),
            Err(SubstepError::NonFiniteSpeed {
                axis: "u",
                index: 0
            })
        );
        assert_eq!(
            clock.select(
                single_cell(),
                &FaceValues {
                    u: vec![0.0],
                    v: vec![0.0, 0.0],
                },
                0.0,
            ),
            Err(SubstepError::Length {
                field: "u",
                expected: 2,
                actual: 1,
            })
        );
    }
}
