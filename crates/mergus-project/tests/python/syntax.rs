//! A file that is not valid Python is a problem of that file (MG-REQ-0611.1).

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{LayoutError, ProjectError, PythonModelError, read_python_lib};
use mergus_sim::SimFileSystem;

use super::{fixture, problems, python, summary};

#[test]
fn mg_req_0611_syntax_error_names_the_file() {
    let found = problems("broken");
    assert_eq!(
        summary(&found),
        [(
            "MG-0626",
            Some(1),
            "`models/broken.py` line 1 column 12: not valid Python: invalid syntax".to_owned()
        )]
    );
    assert_eq!(found[0].column(), Some(12));

    let lib = [fixture!("lib/broken"), fixture!("lib/rates")];
    let fs = SimFileSystem::new();
    for (name, text, _) in lib {
        fs.create_dir_all(Path::new("/project/lib")).unwrap();
        fs.write(
            &Path::new("/project").join(format!("{name}.py")),
            text.as_bytes(),
        )
        .unwrap();
    }
    let read = read_python_lib(&fs, Path::new("/project"), &python(&lib));
    let Err(LayoutError::Invalid(found)) = read else {
        panic!("expected a syntax error, got {read:?}");
    };
    assert_eq!(
        found,
        [ProjectError::Python(PythonModelError::PythonSyntax {
            path: "lib/broken.py".to_owned(),
            line: Some(1),
            column: Some(8),
            message: "'(' was never closed".to_owned(),
        })]
    );
    assert_eq!(found[0].code(), "MG-0626");
    assert_eq!(
        found[0].to_string(),
        "`lib/broken.py` line 1 column 8: not valid Python: '(' was never closed"
    );
}
