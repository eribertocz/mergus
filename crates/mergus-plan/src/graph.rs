//! The graph of models: which model reads which, in an order where every model comes after
//! the models it reads.

use std::collections::{BTreeMap, BTreeSet};

use crate::{PlanError, PlanModel};

/// The models by name, and an order in which each comes after its upstream models.
pub(crate) struct Graph<'a> {
    /// The models by name.
    pub(crate) models: BTreeMap<&'a str, &'a PlanModel>,
    /// Every model name, each after the models it reads; ties in name order.
    pub(crate) order: Vec<&'a str>,
}

/// Where a model is in the depth-first walk.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    Open,
    Done,
}

impl<'a> Graph<'a> {
    /// Orders `models`.
    ///
    /// # Errors
    ///
    /// Fails with [`PlanError::Cycle`] when models read one another in a cycle.
    pub(crate) fn new(models: &'a [PlanModel]) -> Result<Self, PlanError> {
        let models: BTreeMap<&str, &PlanModel> = models
            .iter()
            .map(|model| (model.name.as_str(), model))
            .collect();
        let mut graph = Self {
            models,
            order: Vec::new(),
        };
        let mut marks = BTreeMap::new();
        let names: Vec<&str> = graph.models.keys().copied().collect();
        for name in names {
            graph.visit(name, &mut marks, &mut Vec::new())?;
        }
        Ok(graph)
    }

    /// Visits `name` after its upstream models; `path` holds the models being visited.
    fn visit(
        &mut self,
        name: &'a str,
        marks: &mut BTreeMap<&'a str, Mark>,
        path: &mut Vec<&'a str>,
    ) -> Result<(), PlanError> {
        match marks.get(name) {
            Some(Mark::Done) => return Ok(()),
            Some(Mark::Open) => {
                let start = path.iter().position(|open| *open == name).unwrap_or(0);
                let mut cycle: Vec<String> =
                    path[start..].iter().map(|n| (*n).to_owned()).collect();
                cycle.sort();
                return Err(PlanError::Cycle(cycle));
            }
            None => {}
        }
        marks.insert(name, Mark::Open);
        path.push(name);
        for upstream in self.upstream(name) {
            self.visit(upstream, marks, path)?;
        }
        path.pop();
        marks.insert(name, Mark::Done);
        self.order.push(name);
        Ok(())
    }

    /// The models `name` reads directly, in name order.
    pub(crate) fn upstream(&self, name: &str) -> Vec<&'a str> {
        let Some(model) = self.models.get(name) else {
            return Vec::new();
        };
        model
            .inputs
            .iter()
            .filter_map(|input| self.models.get_key_value(input.as_str()))
            .map(|(name, _)| *name)
            .collect()
    }

    /// Every model upstream of `name`, directly or not.
    pub(crate) fn ancestors(&self, name: &str) -> BTreeSet<&'a str> {
        let mut found = BTreeSet::new();
        let mut pending = self.upstream(name);
        while let Some(next) = pending.pop() {
            if found.insert(next) {
                pending.extend(self.upstream(next));
            }
        }
        found
    }

    /// The depth of each selected model: the most selected models on a chain of models
    /// upstream of it (MG-REQ-0744.3). A selected model with no selected model upstream has
    /// depth 0.
    pub(crate) fn depths(&self, selected: &BTreeSet<String>) -> BTreeMap<&'a str, usize> {
        let mut chains: BTreeMap<&str, usize> = BTreeMap::new();
        let mut depths = BTreeMap::new();
        for name in &self.order {
            let depth = self
                .upstream(name)
                .iter()
                .map(|upstream| chains.get(upstream).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);
            let own = usize::from(selected.contains(*name));
            chains.insert(name, depth + own);
            if own == 1 {
                depths.insert(*name, depth);
            }
        }
        depths
    }
}
