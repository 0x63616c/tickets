//! One board move: unfiltered ranking and sequential, compensating persistence.
//! Writes are not atomic. A rejected forward write is never replayed; every
//! completed write is compensated in its original order, even after a failure.
use tickets_core::domain::Status;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MoveRow {
    pub id: i64,
    pub project_id: i64,
    pub stored_status: Status,
    pub displayed_status: Status,
    /// The legacy label still affects presentation after a later move to Todo.
    pub later: bool,
    pub rank: f64,
}

pub(crate) trait OrderWriter {
    async fn patch_order(&mut self, id: i64, status: Status, rank: f64) -> Result<(), String>;
}

#[derive(Debug, PartialEq)]
pub(crate) enum CommitOutcome {
    Saved,
    Restored(String),
    NeedsReload(String),
}

#[derive(Clone, Copy)]
pub(crate) enum Placement {
    End,
    Before(i64),
    After(i64),
}

#[derive(Clone)]
pub(crate) struct BoardMove {
    initial: Vec<MoveRow>,
    current: Vec<MoveRow>,
    project_id: i64,
    ticket_id: i64,
}

impl BoardMove {
    /// Rows must include the full collection, including tickets hidden by filters.
    pub(crate) fn begin(rows: Vec<MoveRow>, project_id: i64, ticket_id: i64) -> Self {
        Self {
            initial: rows.clone(),
            current: rows,
            project_id,
            ticket_id,
        }
    }

    /// Explicitly sorted columns change status only; manually ordered columns
    /// rank the entire destination, inserting before/after the target or appending.
    pub(crate) fn place(
        &mut self,
        status: Status,
        placement: Placement,
        manual_order: bool,
    ) -> &[MoveRow] {
        let Some(moving) = self
            .current
            .iter()
            .position(|row| row.id == self.ticket_id && row.project_id == self.project_id)
        else {
            return &self.current;
        };
        let mut column: Vec<_> = self
            .current
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                row.project_id == self.project_id
                    && row.displayed_status == status
                    && row.id != self.ticket_id
            })
            .map(|(index, _)| index)
            .collect();
        column.sort_by(|a, b| {
            self.current[*a]
                .rank
                .total_cmp(&self.current[*b].rank)
                .then(self.current[*a].id.cmp(&self.current[*b].id))
        });
        let at = match placement {
            Placement::End => column.len(),
            Placement::Before(id) | Placement::After(id) => column
                .iter()
                .position(|i| self.current[*i].id == id)
                .map(|at| at + usize::from(matches!(placement, Placement::After(_))))
                .unwrap_or(column.len()),
        };
        column.insert(at, moving);
        self.current[moving].stored_status = status;
        // Preserve the legacy later presentation across repeated placements.
        let legacy_later = self.current[moving].later;
        self.current[moving].displayed_status = if legacy_later && status == Status::Todo {
            Status::Backlog
        } else {
            status
        };
        if manual_order {
            for (index, row) in column.into_iter().enumerate() {
                self.current[row].rank = (index + 1) as f64 * 1024.;
            }
        }
        &self.current
    }

    pub(crate) fn cancel(self) -> Vec<MoveRow> {
        self.initial
    }

    pub(crate) async fn commit(self, writer: &mut impl OrderWriter) -> CommitOutcome {
        let mut saved: Vec<&MoveRow> = vec![];
        for (old, row) in self.initial.iter().zip(&self.current) {
            if old.stored_status == row.stored_status && old.rank == row.rank {
                continue;
            }
            match writer
                .patch_order(row.id, row.stored_status, row.rank)
                .await
            {
                Ok(()) => saved.push(old),
                Err(error) => {
                    let mut restored = true;
                    for old in saved {
                        restored &= writer
                            .patch_order(old.id, old.stored_status, old.rank)
                            .await
                            .is_ok();
                    }
                    return if restored {
                        CommitOutcome::Restored(error)
                    } else {
                        CommitOutcome::NeedsReload(error)
                    };
                }
            }
        }
        CommitOutcome::Saved
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use std::collections::VecDeque;

    fn row(id: i64, status: Status, rank: f64) -> MoveRow {
        MoveRow {
            id,
            project_id: 1,
            stored_status: status,
            displayed_status: status,
            later: false,
            rank,
        }
    }
    fn session(rows: Vec<MoveRow>, ticket: i64) -> BoardMove {
        BoardMove::begin(rows, 1, ticket)
    }
    fn ranks(rows: &[MoveRow], status: Status) -> Vec<(i64, f64)> {
        let mut column: Vec<_> = rows
            .iter()
            .filter(|r| r.project_id == 1 && r.displayed_status == status)
            .map(|r| (r.id, r.rank))
            .collect();
        column.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        column
    }

    #[derive(Default)]
    struct ScriptedWriter {
        results: VecDeque<Result<(), String>>,
        attempted: Vec<(i64, Status, f64)>,
        remote: Vec<MoveRow>,
    }
    impl OrderWriter for ScriptedWriter {
        async fn patch_order(&mut self, id: i64, status: Status, rank: f64) -> Result<(), String> {
            self.attempted.push((id, status, rank));
            // Yield once: commit must await completion before starting another write.
            let mut pending = true;
            std::future::poll_fn(|cx| {
                if pending {
                    pending = false;
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                } else {
                    std::task::Poll::Ready(())
                }
            })
            .await;
            self.results.pop_front().unwrap_or(Ok(()))?;
            if let Some(row) = self.remote.iter_mut().find(|r| r.id == id) {
                row.stored_status = status;
                row.displayed_status = if row.later && status == Status::Todo {
                    Status::Backlog
                } else {
                    status
                };
                row.rank = rank;
            }
            Ok(())
        }
    }

    #[test]
    fn same_column_before_and_append_with_numeric_id_ties() {
        let rows = vec![
            row(20, Status::Todo, 7.),
            row(3, Status::Todo, 7.),
            row(10, Status::Todo, 7.),
        ];
        let mut movement = session(rows.clone(), 20);
        assert_eq!(
            ranks(
                movement.place(Status::Todo, Placement::Before(10), true),
                Status::Todo
            ),
            vec![(3, 1024.), (20, 2048.), (10, 3072.)]
        );
        assert_eq!(
            ranks(
                movement.place(Status::Todo, Placement::End, true),
                Status::Todo
            ),
            vec![(3, 1024.), (10, 2048.), (20, 3072.)]
        );
        assert_eq!(movement.cancel(), rows);
    }

    #[test]
    fn cross_column_includes_hidden_rows_and_isolates_projects() {
        let mut other = row(9, Status::Done, 1.);
        other.project_id = 2;
        let rows = vec![
            row(1, Status::Todo, 5.),
            row(2, Status::Done, 8.),
            row(3, Status::Done, 9.),
            other.clone(),
        ];
        let mut movement = session(rows, 1);
        // Ticket 2 models a filtered-out destination row; there is no filtered
        // collection at the move seam, so it must retain its place before 3.
        let preview = movement.place(Status::Done, Placement::Before(3), true);
        assert_eq!(
            ranks(preview, Status::Done),
            vec![(2, 1024.), (1, 2048.), (3, 3072.)]
        );
        assert_eq!(preview[3], other);
        assert_eq!(preview[0].stored_status, Status::Done);
    }

    #[test]
    fn after_target_keeps_hidden_following_tickets_in_order() {
        let mut movement = session(
            vec![
                row(1, Status::Todo, 1024.),
                row(2, Status::Todo, 2048.),
                row(3, Status::Todo, 3072.),
                row(4, Status::Todo, 4096.),
            ],
            1,
        );
        // The keyboard can identify visible ticket 2 without knowing that
        // filtered-out ticket 3 follows it in the complete destination.
        assert_eq!(
            ranks(
                movement.place(Status::Todo, Placement::After(2), true),
                Status::Todo
            ),
            vec![(2, 1024.), (1, 2048.), (3, 3072.), (4, 4096.)]
        );
    }

    #[test]
    fn empty_destination_and_unknown_before_append() {
        let mut movement = session(vec![row(1, Status::Todo, 7.)], 1);
        assert_eq!(
            ranks(
                movement.place(Status::Done, Placement::Before(999), true),
                Status::Done
            ),
            vec![(1, 1024.)]
        );
        let mut movement = session(vec![row(1, Status::Todo, 7.), row(2, Status::Done, 2.)], 1);
        assert_eq!(
            ranks(
                movement.place(Status::Done, Placement::Before(999), true),
                Status::Done
            ),
            vec![(2, 1024.), (1, 2048.)]
        );
    }

    #[test]
    fn sorted_destination_changes_only_status() {
        let mut movement = session(vec![row(1, Status::Todo, 7.), row(2, Status::Done, 9.)], 1);
        let preview = movement.place(Status::Done, Placement::Before(2), false);
        assert_eq!(preview[0], row(1, Status::Done, 7.));
        assert_eq!(preview[1], row(2, Status::Done, 9.));
    }

    #[test]
    fn noop_cancel_and_wrong_project_do_not_write() {
        let original = vec![row(1, Status::Todo, 1024.), row(2, Status::Todo, 2048.)];
        let mut movement = session(original.clone(), 1);
        movement.place(Status::Todo, Placement::Before(2), true);
        let mut writer = ScriptedWriter::default();
        assert_eq!(block_on(movement.commit(&mut writer)), CommitOutcome::Saved);
        assert!(writer.attempted.is_empty());
        let mut movement = BoardMove::begin(original.clone(), 2, 1);
        assert_eq!(movement.place(Status::Done, Placement::End, true), original);
        assert_eq!(movement.cancel(), original);
        let movement = session(original, 1);
        assert_eq!(block_on(movement.commit(&mut writer)), CommitOutcome::Saved);
        assert!(writer.attempted.is_empty());
    }

    #[test]
    fn legacy_later_preserves_stored_status_until_it_is_moved() {
        let mut legacy = row(2, Status::Todo, 4.);
        legacy.displayed_status = Status::Backlog;
        legacy.later = true;
        let mut movement = session(vec![row(1, Status::Backlog, 3.), legacy.clone()], 1);
        let preview = movement.place(Status::Backlog, Placement::End, true);
        assert_eq!(preview[1].stored_status, Status::Todo);
        assert_eq!(preview[1].displayed_status, Status::Backlog);
        let mut writer = ScriptedWriter::default();
        assert_eq!(block_on(movement.commit(&mut writer)), CommitOutcome::Saved);
        assert_eq!(
            writer.attempted,
            vec![(1, Status::Backlog, 2048.), (2, Status::Todo, 1024.)]
        );
        let mut resolved = row(3, Status::Done, 8.);
        resolved.later = true;
        let mut movement = session(vec![resolved], 3);
        assert_eq!(
            movement.place(Status::Todo, Placement::End, true)[0].displayed_status,
            Status::Backlog
        );
        let mut movement = session(vec![legacy], 2);
        assert_eq!(
            movement.place(Status::Done, Placement::End, true)[0].stored_status,
            Status::Done
        );
        // The label remains present in the UI; moving back to Todo displays Backlog.
        assert_eq!(
            movement.place(Status::Todo, Placement::End, true)[0].displayed_status,
            Status::Backlog
        );
        assert_eq!(
            movement.place(Status::Backlog, Placement::End, true)[0].stored_status,
            Status::Backlog
        );
    }

    fn changed_move() -> (Vec<MoveRow>, BoardMove) {
        // Deliberately not rank/ID order: writes must follow collection order.
        let rows = vec![
            row(3, Status::Backlog, 3.),
            row(1, Status::Todo, 1.),
            row(2, Status::Todo, 2.),
        ];
        let mut movement = session(rows.clone(), 3);
        movement.place(Status::Todo, Placement::Before(1), true);
        (rows, movement)
    }

    #[test]
    fn successful_commit_writes_sequentially_in_collection_order() {
        let (rows, movement) = changed_move();
        let mut writer = ScriptedWriter {
            remote: rows,
            ..Default::default()
        };
        assert_eq!(block_on(movement.commit(&mut writer)), CommitOutcome::Saved);
        assert_eq!(
            writer.attempted,
            vec![
                (3, Status::Todo, 1024.),
                (1, Status::Todo, 2048.),
                (2, Status::Todo, 3072.)
            ]
        );
        assert_eq!(
            ranks(&writer.remote, Status::Todo),
            vec![(3, 1024.), (1, 2048.), (2, 3072.)]
        );
    }

    #[test]
    fn every_rejected_prefix_restores_completed_writes_in_forward_order() {
        for prefix in 0..3 {
            let (rows, movement) = changed_move();
            let mut results: VecDeque<_> = (0..prefix).map(|_| Ok(())).collect();
            results.push_back(Err("rejected".into()));
            let mut writer = ScriptedWriter {
                results,
                remote: rows.clone(),
                ..Default::default()
            };
            assert_eq!(
                block_on(movement.commit(&mut writer)),
                CommitOutcome::Restored("rejected".into())
            );
            assert_eq!(writer.remote, rows);
            assert_eq!(writer.attempted.len(), prefix + 1 + prefix);
            assert_eq!(
                writer
                    .attempted
                    .iter()
                    .take(prefix + 1)
                    .map(|w| w.0)
                    .collect::<Vec<_>>(),
                [3, 1, 2][..=prefix]
            );
            assert_eq!(
                &writer.attempted[prefix + 1..],
                &rows[..prefix]
                    .iter()
                    .map(|r| (r.id, r.stored_status, r.rank))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn every_compensation_failure_requires_reload_and_does_not_stop_restoration() {
        for failures in 1..4 {
            let (rows, movement) = changed_move();
            let mut results = VecDeque::from([Ok(()), Ok(()), Err("forward".into())]);
            for index in 0..2 {
                results.push_back(if failures & (1 << index) != 0 {
                    Err("compensation".into())
                } else {
                    Ok(())
                });
            }
            let mut writer = ScriptedWriter {
                results,
                remote: rows.clone(),
                ..Default::default()
            };
            assert_eq!(
                block_on(movement.commit(&mut writer)),
                CommitOutcome::NeedsReload("forward".into())
            );
            assert_eq!(writer.attempted.len(), 5);
            assert_eq!(
                &writer.attempted[3..],
                &[(3, Status::Backlog, 3.), (1, Status::Todo, 1.)]
            );
            assert_ne!(writer.remote, rows);
        }
    }
}
