//! The cycles between models of any language (MG-REQ-0703). `mergus-project` finds those
//! among `.sql` models; a cycle through a `.py` model is found here, on every model's inputs.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mergus_project::ProjectError;

/// One model of the graph: the path of its file, and each model it reads with the line of
/// its reference.
pub(crate) struct Node {
    pub(crate) path: String,
    pub(crate) reads: BTreeMap<String, usize>,
}

/// Every model of the project, by name.
pub(crate) type Graph = BTreeMap<String, Node>;

/// The models reachable from `start` following `next`, `start` excluded unless on a cycle.
fn reachable<'g>(start: &'g str, next: &dyn Fn(&str) -> Vec<&'g str>) -> BTreeSet<&'g str> {
    let mut seen = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(name) = pending.pop() {
        for child in next(name) {
            if seen.insert(child) {
                pending.push(child);
            }
        }
    }
    seen
}

/// The groups of models that reach one another and hold a cycle, each sorted, ordered by
/// their first model.
fn cyclic_groups(graph: &Graph) -> Vec<BTreeSet<&str>> {
    let forward = |name: &str| -> Vec<&str> {
        graph.get(name).map_or_else(Vec::new, |node| {
            node.reads
                .keys()
                .filter(|to| graph.contains_key(*to))
                .map(String::as_str)
                .collect()
        })
    };
    let backward = |name: &str| -> Vec<&str> {
        graph
            .iter()
            .filter(|(_, node)| node.reads.contains_key(name))
            .map(|(from, _)| from.as_str())
            .collect()
    };
    let mut assigned = BTreeSet::new();
    let mut groups = Vec::new();
    for name in graph.keys() {
        if assigned.contains(name.as_str()) {
            continue;
        }
        let ahead = reachable(name, &forward);
        if !ahead.contains(name.as_str()) {
            continue;
        }
        let behind = reachable(name, &backward);
        let group: BTreeSet<&str> = ahead.intersection(&behind).copied().collect();
        assigned.extend(group.iter().copied());
        groups.push(group);
    }
    groups
}

/// The shortest cycle of `group` through its first model, neighbors taken in name order.
fn cycle<'g>(graph: &'g Graph, group: &BTreeSet<&'g str>) -> Vec<&'g str> {
    let Some(&start) = group.first() else {
        return Vec::new();
    };
    let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
    let mut queue = VecDeque::from([start]);
    while let Some(name) = queue.pop_front() {
        let reads = graph.get(name).map(|node| &node.reads);
        for child in reads.into_iter().flat_map(BTreeMap::keys) {
            let child = child.as_str();
            if !group.contains(child) {
                continue;
            }
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

/// One [`ProjectError::ModelCycle`] for each group of models of `graph` that read one another
/// in a cycle: its models in reference order from the first in name order, each with the
/// path of its file and the line of its reference to the next (MG-REQ-0375).
pub(crate) fn cycles(graph: &Graph) -> Vec<ProjectError> {
    cyclic_groups(graph)
        .iter()
        .map(|group| {
            let models = cycle(graph, group);
            let next = models.iter().skip(1).chain(models.first());
            let references = models
                .iter()
                .zip(next)
                .filter_map(|(name, to)| {
                    let node = graph.get(*name)?;
                    Some((node.path.clone(), *node.reads.get(*to)?))
                })
                .collect();
            ProjectError::ModelCycle {
                models: models.iter().map(|name| (*name).to_owned()).collect(),
                references,
            }
        })
        .collect()
}

/// The line, from 1, of the first word of `text` equal to `name`, ignoring case; 1 when there
/// is none. The public API of `mergus-project` gives a `.sql` model's references without
/// their positions (#300).
pub(crate) fn reference_line(text: &str, name: &str) -> usize {
    text.lines()
        .position(|line| {
            line.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|word| word.eq_ignore_ascii_case(name))
        })
        .map_or(1, |index| index + 1)
}

#[cfg(test)]
mod tests {
    use super::reference_line;

    #[test]
    fn a_reference_is_found_by_its_word() {
        assert_eq!(
            reference_line("SELECT *\nFROM b_orders\nJOIN b ON true", "b"),
            3
        );
        assert_eq!(reference_line("SELECT * FROM B", "b"), 1);
        assert_eq!(reference_line("SELECT 1", "b"), 1);
    }
}
