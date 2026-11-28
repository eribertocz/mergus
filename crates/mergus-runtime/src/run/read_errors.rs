//! The errors of stage 1 grouped by file, in layout order (MG-REQ-0702.1).

use std::cmp::Ordering;

use mergus_project::ProjectError;

/// The errors of one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileErrors {
    /// The file, relative to the project root, separated by `/`.
    pub path: String,
    /// Its errors, in the order the file states them: those of the whole file first, then
    /// by line and column, each pass's order kept between equals.
    pub errors: Vec<ProjectError>,
}

/// The rank of a top folder in the walking order of the layout: `models/`, `sources/`,
/// `seeds/`, then every other file.
fn top_rank(path: &str) -> u8 {
    match path.split('/').next() {
        Some("models") => 0,
        Some("sources") => 1,
        Some("seeds") => 2,
        _ => 3,
    }
}

/// Compares two paths in the walking order of the layout: by top folder, then entry by
/// entry, each folder's entries sorted by name.
fn walking_order(left: &str, right: &str) -> Ordering {
    top_rank(left)
        .cmp(&top_rank(right))
        .then_with(|| left.split('/').cmp(right.split('/')))
}

/// Groups `errors` by file, in walking order, keeping in each file the order it states them.
pub(crate) fn group(errors: Vec<ProjectError>) -> Vec<FileErrors> {
    let mut groups: Vec<FileErrors> = Vec::new();
    for error in errors {
        let path = error.path().to_owned();
        match groups.iter_mut().find(|group| group.path == path) {
            Some(group) => group.errors.push(error),
            None => groups.push(FileErrors {
                path,
                errors: vec![error],
            }),
        }
    }
    groups.sort_by(|left, right| walking_order(&left.path, &right.path));
    for group in &mut groups {
        group
            .errors
            .sort_by_key(|error| (error.line(), error.column()));
    }
    groups
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::walking_order;

    #[test]
    fn paths_follow_the_walk_of_the_layout() {
        assert_eq!(
            walking_order("models/b.sql", "sources/a.toml"),
            Ordering::Less
        );
        assert_eq!(
            walking_order("sources/a.toml", "seeds/a.csv"),
            Ordering::Less
        );
        assert_eq!(walking_order("seeds/a.csv", "project.toml"), Ordering::Less);
        assert_eq!(
            walking_order("models/a/z.sql", "models/b.sql"),
            Ordering::Less
        );
    }
}
