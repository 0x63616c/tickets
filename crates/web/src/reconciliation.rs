//! In-process live queue and scheduling policy. Time and browser readiness are inputs;
//! callers execute returned actions without owning any queue or cooldown state.
use std::collections::BTreeMap;
use tickets_core::live::ChangeEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Route {
    Inactive,
    Summary,
    Collection,
}

impl Route {
    pub(crate) fn from_path(path: &str) -> Self {
        match path {
            "/" | "/home" | "/analytics" => Self::Summary,
            "/board" | "/list" | "/ticket" | "/projects" | "/mission" => Self::Collection,
            _ if path.starts_with("/t/") => Self::Collection,
            _ => Self::Inactive,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Readiness {
    pub route: Route,
    pub online: bool,
    pub visible: bool,
    pub signed_in: bool,
    pub busy: bool,
    pub dragged: bool,
    pub composer: bool,
    pub refreshing: bool,
    pub editing: bool,
}

impl Readiness {
    fn can_schedule(self) -> bool {
        self.route != Route::Inactive
            && self.online
            && self.visible
            && self.signed_in
            && !self.busy
            && !self.dragged
            && !self.composer
            && !self.refreshing
    }
}

pub(crate) enum Input {
    Notice(ChangeEvent),
    Recover,
    ReadinessChanged,
    TimerElapsed,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Action {
    ScheduleTimer(u32),
    Reconcile(Vec<ChangeEvent>),
    CatchUp,
    InvalidateTickets,
}

#[derive(Default)]
pub(crate) struct Reconciler {
    events: BTreeMap<(String, i64), ChangeEvent>,
    recover: bool,
    scheduled: bool,
    last_reload: Option<u64>,
}

impl Reconciler {
    /// An elapsed timer is delivered once per ScheduleTimer action. Readiness is
    /// sampled again on completion; edits block dispatch, but not scheduling.
    /// Deferred events retain their latest notice and recovery takes precedence.
    pub(crate) fn advance(
        &mut self,
        input: Input,
        now_ms: u64,
        readiness: Readiness,
    ) -> Vec<Action> {
        match input {
            Input::Notice(event) => {
                self.events.insert((event.entity.clone(), event.id), event);
            }
            Input::Recover => self.recover = true,
            Input::ReadinessChanged => {}
            Input::TimerElapsed => {
                if !self.scheduled {
                    return vec![];
                }
                self.scheduled = false;
                if readiness.can_schedule() && !readiness.editing {
                    self.last_reload = Some(now_ms);
                    let pending = std::mem::take(&mut self.events)
                        .into_values()
                        .collect::<Vec<_>>();
                    let recover = std::mem::take(&mut self.recover);
                    return vec![if recover || pending.iter().any(|event| event.id == 0) {
                        if readiness.route == Route::Summary {
                            Action::InvalidateTickets
                        } else {
                            Action::CatchUp
                        }
                    } else {
                        Action::Reconcile(pending)
                    }];
                }
            }
        }
        if (self.recover || !self.events.is_empty()) && !self.scheduled && readiness.can_schedule()
        {
            self.scheduled = true;
            // A backwards wall-clock adjustment gets a full cooldown, as before.
            let delay = self.last_reload.map_or(5_000, |last| {
                30_000_u64
                    .saturating_sub(now_ms.saturating_sub(last))
                    .max(5_000) as u32
            });
            vec![Action::ScheduleTimer(delay)]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(path: &str) -> Readiness {
        Readiness {
            route: Route::from_path(path),
            online: true,
            visible: true,
            signed_in: true,
            busy: false,
            dragged: false,
            composer: false,
            refreshing: false,
            editing: false,
        }
    }

    fn event(entity: &str, id: i64, action: &str) -> ChangeEvent {
        ChangeEvent {
            project_id: 1,
            entity: entity.into(),
            id,
            action: action.into(),
        }
    }

    #[test]
    fn batches_latest_notices_by_entity_and_id_including_notices_during_wait() {
        let mut r = Reconciler::default();
        let ready = ready("/board");
        let first = event("tickets", 7, "create");
        let latest = event("tickets", 7, "update");
        let label = event("labels", 7, "delete");
        let other = event("tickets", 8, "update");
        assert_eq!(
            r.advance(Input::Notice(first), 0, ready),
            vec![Action::ScheduleTimer(5_000)]
        );
        for notice in [latest.clone(), label.clone(), other.clone()] {
            assert!(r.advance(Input::Notice(notice), 4_000, ready).is_empty());
        }
        assert!(r.advance(Input::ReadinessChanged, 4_999, ready).is_empty());
        assert_eq!(
            r.advance(Input::TimerElapsed, 5_000, ready),
            vec![Action::Reconcile(vec![label, latest, other])]
        );
        assert!(r.advance(Input::TimerElapsed, 5_000, ready).is_empty());
        assert!(
            r.advance(Input::ReadinessChanged, 100_000, ready)
                .is_empty()
        );
    }

    #[test]
    fn cooldown_uses_completion_time_and_saturates_backwards_clock_movement() {
        for (now, delay) in [
            (6_000, 29_000),
            (30_000, 5_000),
            (35_000, 5_000),
            (100_000, 5_000),
            (4_000, 30_000),
        ] {
            let mut r = Reconciler::default();
            let ready = ready("/board");
            assert_eq!(
                r.advance(Input::Recover, 0, ready),
                vec![Action::ScheduleTimer(5_000)]
            );
            assert_eq!(
                r.advance(Input::TimerElapsed, 5_000, ready),
                vec![Action::CatchUp]
            );
            assert_eq!(
                r.advance(Input::Recover, now, ready),
                vec![Action::ScheduleTimer(delay)]
            );
            assert_eq!(
                r.advance(Input::TimerElapsed, now + u64::from(delay), ready),
                vec![Action::CatchUp]
            );
            assert!(
                r.advance(Input::ReadinessChanged, now + 100_000, ready)
                    .is_empty()
            );
        }
    }

    #[test]
    fn notices_during_cooldown_join_the_existing_wait_without_extending_it() {
        let mut r = Reconciler::default();
        let ready = ready("/board");
        assert_eq!(
            r.advance(Input::Recover, 0, ready),
            vec![Action::ScheduleTimer(5_000)]
        );
        assert_eq!(
            r.advance(Input::TimerElapsed, 5_000, ready),
            vec![Action::CatchUp]
        );
        assert_eq!(
            r.advance(Input::Notice(event("tickets", 7, "create")), 6_000, ready),
            vec![Action::ScheduleTimer(29_000)]
        );
        let latest = event("tickets", 7, "update");
        assert!(
            r.advance(Input::Notice(latest.clone()), 34_000, ready)
                .is_empty()
        );
        assert_eq!(
            r.advance(Input::TimerElapsed, 35_000, ready),
            vec![Action::Reconcile(vec![latest])]
        );
        assert!(r.advance(Input::TimerElapsed, 35_000, ready).is_empty());
    }

    #[test]
    fn each_readiness_gate_retains_notices_before_scheduling_and_at_completion() {
        let ready = ready("/list");
        let blocked = [
            Readiness {
                route: Route::Inactive,
                ..ready
            },
            Readiness {
                online: false,
                ..ready
            },
            Readiness {
                visible: false,
                ..ready
            },
            Readiness {
                signed_in: false,
                ..ready
            },
            Readiness {
                busy: true,
                ..ready
            },
            Readiness {
                dragged: true,
                ..ready
            },
            Readiness {
                composer: true,
                ..ready
            },
            Readiness {
                refreshing: true,
                ..ready
            },
        ];
        for blocked in blocked {
            let mut r = Reconciler::default();
            let notice = event("tickets", 7, "update");
            assert!(
                r.advance(Input::Notice(notice.clone()), 0, blocked)
                    .is_empty()
            );
            assert!(r.advance(Input::ReadinessChanged, 500, blocked).is_empty());
            assert_eq!(
                r.advance(Input::ReadinessChanged, 1_000, ready),
                vec![Action::ScheduleTimer(5_000)]
            );
            assert!(r.advance(Input::TimerElapsed, 6_000, blocked).is_empty());
            assert_eq!(
                r.advance(Input::ReadinessChanged, 7_000, ready),
                vec![Action::ScheduleTimer(5_000)]
            );
            assert_eq!(
                r.advance(Input::TimerElapsed, 12_000, ready),
                vec![Action::Reconcile(vec![notice])]
            );
            assert!(r.advance(Input::TimerElapsed, 12_000, ready).is_empty());
        }
    }

    #[test]
    fn edits_only_gate_completion_and_deferred_wait_keeps_new_notices() {
        let mut r = Reconciler::default();
        let ready = ready("/ticket");
        let editing = Readiness {
            editing: true,
            ..ready
        };
        let notice = event("tickets", 7, "update");
        let latest = event("tickets", 7, "delete");
        assert_eq!(
            r.advance(Input::Notice(notice), 0, ready),
            vec![Action::ScheduleTimer(5_000)]
        );
        assert_eq!(
            r.advance(Input::TimerElapsed, 5_000, editing),
            vec![Action::ScheduleTimer(5_000)]
        );
        assert!(
            r.advance(Input::Notice(latest.clone()), 6_000, editing)
                .is_empty()
        );
        assert!(r.advance(Input::ReadinessChanged, 7_000, ready).is_empty());
        assert_eq!(
            r.advance(Input::TimerElapsed, 10_000, ready),
            vec![Action::Reconcile(vec![latest])]
        );
        // Scheduling while already editing is intentional too.
        assert_eq!(
            r.advance(Input::Recover, 11_000, editing),
            vec![Action::ScheduleTimer(29_000)]
        );
        assert_eq!(
            r.advance(Input::TimerElapsed, 40_000, editing),
            vec![Action::ScheduleTimer(5_000)]
        );
        assert_eq!(
            r.advance(Input::TimerElapsed, 45_000, ready),
            vec![Action::CatchUp]
        );
    }

    #[test]
    fn recovery_and_zero_id_take_precedence_on_the_completion_route() {
        for path in [
            "/",
            "/home",
            "/analytics",
            "/board",
            "/list",
            "/ticket",
            "/projects",
            "/mission",
            "/t/TIK-110",
        ] {
            for recovery in [false, true] {
                let mut r = Reconciler::default();
                let initial = ready("/board");
                let completion = ready(path);
                let notice = event("tickets", 7, "update");
                assert_eq!(
                    r.advance(Input::Notice(notice.clone()), 0, initial),
                    vec![Action::ScheduleTimer(5_000)]
                );
                if recovery {
                    for _ in 0..3 {
                        assert!(r.advance(Input::Recover, 1_000, initial).is_empty());
                    }
                } else {
                    assert!(
                        r.advance(
                            Input::Notice(event("projects", 0, "update")),
                            1_000,
                            initial
                        )
                        .is_empty()
                    );
                }
                let expected = if completion.route == Route::Summary {
                    Action::InvalidateTickets
                } else {
                    Action::CatchUp
                };
                assert_eq!(
                    r.advance(Input::TimerElapsed, 5_000, completion),
                    vec![expected]
                );
                // Recovery drains the notices and does not leak into the next batch.
                assert!(
                    r.advance(Input::ReadinessChanged, 6_000, completion)
                        .is_empty()
                );
                assert_eq!(
                    r.advance(Input::Notice(notice.clone()), 6_000, completion),
                    vec![Action::ScheduleTimer(29_000)]
                );
                assert_eq!(
                    r.advance(Input::TimerElapsed, 35_000, completion),
                    vec![Action::Reconcile(vec![notice])]
                );
            }
        }
    }

    #[test]
    fn repeated_recovery_invalidates_unloaded_summary_after_hidden_timer_deferral() {
        let mut r = Reconciler::default();
        let ready = ready("/analytics");
        let hidden = Readiness {
            visible: false,
            ..ready
        };
        assert_eq!(
            r.advance(Input::Recover, 0, ready),
            vec![Action::ScheduleTimer(5_000)]
        );
        assert!(r.advance(Input::TimerElapsed, 5_000, hidden).is_empty());
        for _ in 0..3 {
            assert!(r.advance(Input::Recover, 6_000, hidden).is_empty());
        }
        assert_eq!(
            r.advance(Input::ReadinessChanged, 7_000, ready),
            vec![Action::ScheduleTimer(5_000)]
        );
        assert_eq!(
            r.advance(Input::TimerElapsed, 12_000, ready),
            vec![Action::InvalidateTickets]
        );
        assert!(r.advance(Input::TimerElapsed, 12_000, ready).is_empty());
    }

    #[test]
    fn inactive_routes_do_not_schedule_and_clean_readiness_does_not_fetch() {
        for path in [
            "/settings",
            "/settings/account",
            "/design",
            "/design/board",
            "/login",
            "/unknown",
            "/t",
        ] {
            let mut r = Reconciler::default();
            assert_eq!(Route::from_path(path), Route::Inactive);
            assert!(r.advance(Input::Recover, 0, ready(path)).is_empty());
        }
        let mut r = Reconciler::default();
        assert!(
            r.advance(Input::ReadinessChanged, 0, ready("/home"))
                .is_empty()
        );
        assert!(
            r.advance(Input::TimerElapsed, 5_000, ready("/home"))
                .is_empty()
        );
    }
}
