//! Edges point from the blocker to the blocked ticket.
use crate::domain::Dependency;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyError {
    SelfDependency,
    Cycle,
    Duplicate,
}

#[derive(Debug, Default)]
pub struct DependencyGraph {
    edges: Vec<Dependency>,
}
impl DependencyGraph {
    pub fn new(edges: impl IntoIterator<Item = Dependency>) -> Result<Self, DependencyError> {
        let mut graph = Self::default();
        for edge in edges {
            graph.insert(edge)?;
        }
        Ok(graph)
    }
    pub fn blocks(&self, id: i64) -> Vec<i64> {
        self.edges
            .iter()
            .filter(|e| e.blocker_id == id)
            .map(|e| e.blocked_id)
            .collect()
    }
    pub fn blocked_by(&self, id: i64) -> Vec<i64> {
        self.edges
            .iter()
            .filter(|e| e.blocked_id == id)
            .map(|e| e.blocker_id)
            .collect()
    }
    pub fn check(&self, edge: Dependency) -> Result<(), DependencyError> {
        if edge.blocker_id == edge.blocked_id {
            return Err(DependencyError::SelfDependency);
        }
        if self.edges.contains(&edge) {
            return Err(DependencyError::Duplicate);
        }
        let mut pending = vec![edge.blocked_id];
        let mut seen = HashSet::new();
        while let Some(id) = pending.pop() {
            if id == edge.blocker_id {
                return Err(DependencyError::Cycle);
            }
            if seen.insert(id) {
                pending.extend(self.blocks(id));
            }
        }
        Ok(())
    }
    pub fn insert(&mut self, edge: Dependency) -> Result<(), DependencyError> {
        self.check(edge)?;
        self.edges.push(edge);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn edge(a: i64, b: i64) -> Dependency {
        Dependency {
            blocker_id: a,
            blocked_id: b,
        }
    }
    #[test]
    fn directions_and_rejections() {
        let mut g = DependencyGraph::new([edge(1, 2), edge(2, 3), edge(1, 4), edge(4, 3)]).unwrap();
        assert_eq!(g.blocks(1), vec![2, 4]);
        assert_eq!(g.blocked_by(3), vec![2, 4]);
        assert!(g.blocks(99).is_empty());
        assert_eq!(g.insert(edge(1, 1)), Err(DependencyError::SelfDependency));
        assert_eq!(g.insert(edge(1, 2)), Err(DependencyError::Duplicate));
        for e in [edge(2, 1), edge(3, 1), edge(3, 4)] {
            assert_eq!(g.insert(e), Err(DependencyError::Cycle));
        }
        g.insert(edge(8, 9)).unwrap();
        g.insert(edge(3, 8)).unwrap();
        assert_eq!(g.insert(edge(9, 1)), Err(DependencyError::Cycle));
        assert!(DependencyGraph::new([edge(1, 2), edge(2, 1)]).is_err());
    }
    #[test]
    fn long_chain() {
        let mut g = DependencyGraph::new((1..1000).map(|i| edge(i, i + 1))).unwrap();
        assert_eq!(g.insert(edge(1000, 1)), Err(DependencyError::Cycle));
        g.insert(edge(1000, 1001)).unwrap();
    }
}
