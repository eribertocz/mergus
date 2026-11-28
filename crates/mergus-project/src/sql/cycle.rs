//! The reference cycles between models (MG-REQ-0375).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::Position;
use crate::{InputSource, Model, ProjectError, PythonModel};

/// The graph of the references between models, `.sql` and `.py` together (MG-REQ-0375.1):
/// for each model, by lowercase name, the path of its file and the models it reads with the
/// line of the first reference to each (MG-REQ-0375.2).
struct Graph<'a> {
    nodes: BTreeMap<String, (&'a str, BTreeMap<&'a str, usize>)>,
}

/// One model and the line of its first reference to each table it reads.
type Reads<'a> = (&'a str, &'a str, BTreeMap<&'a str, usize>);

impl<'a> Graph<'a> {
    /// The graph of the `.sql` models of `found`, each with its resolved references, and of
    /// the `.py` models of `python`, each with the models its parameters read. Raw tables
    /// read nothing, so they close no cycle.
    fn new(
        found: &'a [(&'a Model, BTreeMap<String, Position>)],
        python: &'a [PythonModel],
    ) -> Self {
        let sql = found.iter().map(|(model, references)| {
            let reads = references.iter().map(|(name, at)| (name.as_str(), at.line));
            (model.name.as_str(), model.path.as_str(), reads.collect())
        });
        let python = python.iter().map(|model| {
            (
                model.name.as_str(),
                model.path.as_str(),
                python_reads(model),
            )
        });
        let models: Vec<Reads<'a>> = sql.chain(python).collect();
        let mut nodes = BTreeMap::new();
        for (name, path, _) in &models {
            nodes
                .entry(name.to_lowercase())
                .or_insert((*path, BTreeMap::new()));
        }
        for (name, path, reads) in models {
            let edges: BTreeMap<&str, usize> = reads
                .into_iter()
                .filter(|(to, _)| nodes.contains_key(*to))
                .collect();
            if let Some((own, node)) = nodes.get_mut(&name.to_lowercase())
                && *own == path
            {
                *node = edges;
            }
        }
        Self { nodes }
    }

    /// The models `name` reads, in name order.
    fn next(&self, name: &str) -> impl Iterator<Item = &'a str> + '_ {
        self.nodes
            .get(name)
            .into_iter()
            .flat_map(|(_, edges)| edges.keys().copied())
    }

    /// For each model, the models that read it.
    fn readers(&self) -> BTreeMap<&str, Vec<&str>> {
        let mut readers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (from, (_, edges)) in &self.nodes {
            for to in edges.keys() {
                readers.entry(to).or_default().push(from);
            }
        }
        readers
    }

    /// The models in the order a depth-first walk in name order finishes them.
    fn finish_order(&self) -> Vec<&str> {
        let mut seen = BTreeSet::new();
        let mut order = Vec::new();
        for start in self.nodes.keys() {
            if !seen.insert(start.as_str()) {
                continue;
            }
            let mut stack = vec![(start.as_str(), self.next(start).collect::<Vec<_>>())];
            while let Some((name, pending)) = stack.last_mut() {
                let name = *name;
                match pending.pop() {
                    Some(child) if seen.insert(child) => {
                        let children = self.next(child).collect();
                        stack.push((child, children));
                    }
                    Some(_) => {}
                    None => {
                        order.push(name);
                        stack.pop();
                    }
                }
            }
        }
        order
    }

    /// The groups of models that reach one another (Kosaraju), each sorted, that hold a
    /// cycle: more than one model, or one model that reads itself.
    fn cyclic_groups(&self) -> Vec<BTreeSet<&str>> {
        let readers = self.readers();
        let mut assigned = BTreeSet::new();
        let mut groups = Vec::new();
        for root in self.finish_order().into_iter().rev() {
            if !assigned.insert(root) {
                continue;
            }
            let mut group = BTreeSet::from([root]);
            let mut stack = vec![root];
            while let Some(name) = stack.pop() {
                for &from in readers.get(name).into_iter().flatten() {
                    if assigned.insert(from) {
                        group.insert(from);
                        stack.push(from);
                    }
                }
            }
            let reads_itself = self.next(root).any(|child| child == root);
            if group.len() > 1 || reads_itself {
                groups.push(group);
            }
        }
        groups.sort();
        groups
    }

    /// The shortest cycle of `group` through its first model, neighbors taken in name order.
    fn cycle<'g>(&'g self, group: &BTreeSet<&'g str>) -> Vec<&'g str> {
        let Some(&start) = group.first() else {
            return Vec::new();
        };
        let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
        let mut queue = VecDeque::from([start]);
        while let Some(name) = queue.pop_front() {
            for child in self.next(name).filter(|child| group.contains(child)) {
                if child == start {
                    let mut models = vec![name];
                    let mut current = name;
                    while let Some(&before) = parent.get(current) {
                        models.push(before);
                        current = before;
                    }
                    models.reverse();
                    return models;
                }
                if !parent.contains_key(child) {
                    parent.insert(child, name);
                    queue.push_back(child);
                }
            }
        }
        vec![start]
    }

    /// The error of MG-REQ-0375 for the cycle `models`.
    fn error(&self, models: &[&str]) -> ProjectError {
        let next = models.iter().skip(1).chain(models.first());
        let references = models
            .iter()
            .zip(next)
            .filter_map(|(name, to)| {
                let (path, edges) = self.nodes.get(*name)?;
                Some(((*path).to_owned(), *edges.get(to)?))
            })
            .collect();
        ProjectError::ModelCycle {
            models: models.iter().map(|name| (*name).to_owned()).collect(),
            references,
        }
    }
}

/// The models the parameters of `model` read, each with the line of the first parameter that
/// names it, in the order of the file (MG-REQ-0375.2).
fn python_reads(model: &PythonModel) -> BTreeMap<&str, usize> {
    let mut reads = BTreeMap::new();
    for step in &model.steps {
        for input in &step.inputs {
            if let InputSource::Model(name) = &input.source {
                reads.entry(name.as_str()).or_insert(input.line);
            }
        }
    }
    reads
}

/// Returns one error for each group of models of `found` (`.sql`) and `python` (`.py`) that
/// read one another in a cycle, ordered by the first model of each in name order
/// (MG-REQ-0375).
pub(super) fn cycles(
    found: &[(&Model, BTreeMap<String, Position>)],
    python: &[PythonModel],
) -> Vec<ProjectError> {
    let graph = Graph::new(found, python);
    graph
        .cyclic_groups()
        .iter()
        .map(|group| graph.error(&graph.cycle(group)))
        .collect()
}
