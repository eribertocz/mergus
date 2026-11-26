//! Every command and the arguments it accepts, defined once: the parser of [`crate::args`]
//! reads them and `mergus schema` prints them (MG-REQ-0263, MG-REQ-1348).

/// The option that asks for events on standard output.
pub(crate) const JSON: &str = "--json";

/// The option that shows only errors.
pub(crate) const QUIET: &str = "--quiet";

/// The option that adds a line per stage.
pub(crate) const VERBOSE: &str = "--verbose";

/// The option that lists the files a command would write and writes none (MG-REQ-1350.3).
pub(crate) const DRY_RUN: &str = "--dry-run";

/// The option that removes models from a selection (MG-REQ-1305).
pub(crate) const EXCLUDE: &str = "--exclude";

/// The option that sets `max_parallel` (MG-REQ-1351).
pub(crate) const MAX_PARALLEL: &str = "--max-parallel";

/// The positional words of a selection (MG-REQ-1305).
pub(crate) const SELECTION: &str = "selection";

/// The exit codes every command may end with: success, a project error, an internal error.
const EXIT_CODES: [u8; 3] = [0, 1, 10];

/// A command the binary runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// `mergus add source` (MG-REQ-1304).
    AddSource,
    /// `mergus doctor` (MG-REQ-1004).
    Doctor,
    /// `mergus explain` (MG-REQ-1310).
    Explain,
    /// `mergus init` (MG-REQ-1301).
    Init,
    /// `mergus ls` (MG-REQ-1305).
    Ls,
    /// `mergus new model` (MG-REQ-1304).
    NewModel,
    /// `mergus plan` (MG-REQ-0746).
    Plan,
    /// `mergus schema` (MG-REQ-0263).
    Schema,
    /// `mergus secret set` (MG-REQ-1007).
    SecretSet,
    /// `mergus validate` (MG-REQ-1345).
    Validate,
}

/// Every command, sorted by name (MG-REQ-1348).
pub(crate) const ALL: [Command; 10] = [
    Command::AddSource,
    Command::Doctor,
    Command::Explain,
    Command::Init,
    Command::Ls,
    Command::NewModel,
    Command::Plan,
    Command::Schema,
    Command::SecretSet,
    Command::Validate,
];

/// The names of every command, sorted.
pub(crate) const NAMES: [&str; 10] = [
    "add source",
    "doctor",
    "explain",
    "init",
    "ls",
    "new model",
    "plan",
    "schema",
    "secret set",
    "validate",
];

/// How an argument is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A word without dashes, by position.
    Positional,
    /// `--name value` or `--name=value`.
    Option,
    /// `--name`, without a value.
    Flag,
}

/// The type of the value of an argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Type {
    /// Any text.
    String,
    /// An integer.
    Integer,
    /// A selector of MG-REQ-1305.
    Selection,
    /// A flag.
    Boolean,
}

/// One argument a command accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Argument {
    /// The name: `--name` for an option or a flag, the bare word for a positional argument.
    pub(crate) name: &'static str,
    pub(crate) kind: Kind,
    pub(crate) r#type: Type,
    pub(crate) required: bool,
    /// Whether the argument may be given more than once.
    pub(crate) repeatable: bool,
    /// The value used when the argument is not given.
    pub(crate) default: Option<&'static str>,
    /// The only values allowed; empty when any value is.
    pub(crate) values: &'static [&'static str],
}

const fn flag(name: &'static str) -> Argument {
    Argument {
        name,
        kind: Kind::Flag,
        r#type: Type::Boolean,
        required: false,
        repeatable: false,
        default: None,
        values: &[],
    }
}

const fn option(name: &'static str, default: Option<&'static str>) -> Argument {
    Argument {
        name,
        kind: Kind::Option,
        r#type: Type::String,
        required: false,
        repeatable: false,
        default,
        values: &[],
    }
}

const fn positional(name: &'static str, required: bool) -> Argument {
    Argument {
        name,
        kind: Kind::Positional,
        r#type: Type::String,
        required,
        repeatable: false,
        default: None,
        values: &[],
    }
}

/// The options every command accepts (MG-REQ-0260, MG-REQ-1347).
const GLOBAL: [Argument; 3] = [flag(JSON), flag(QUIET), flag(VERBOSE)];

/// The profiles `init` offers in phase 0.2 (MG-REQ-1349.1).
pub(crate) const PROFILES: [&str; 1] = ["local"];

const ADD_SOURCE: [Argument; 5] = [
    positional("name", true),
    flag(DRY_RUN),
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

const EXPLAIN: [Argument; 4] = [
    positional("argument", true),
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

/// The words of a selection, each a union member (MG-REQ-1305).
const SELECTED: Argument = Argument {
    r#type: Type::Selection,
    repeatable: true,
    ..positional(SELECTION, false)
};

/// The models to leave out of a selection (MG-REQ-1305).
const EXCLUDED: Argument = Argument {
    r#type: Type::Selection,
    ..option(EXCLUDE, None)
};

const LS: [Argument; 5] = [SELECTED, EXCLUDED, GLOBAL[0], GLOBAL[1], GLOBAL[2]];

const PLAN: [Argument; 6] = [
    SELECTED,
    EXCLUDED,
    Argument {
        r#type: Type::Integer,
        ..option(MAX_PARALLEL, Some("4"))
    },
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

const INIT: [Argument; 8] = [
    option("--name", None),
    Argument {
        values: &PROFILES,
        ..option("--profile", Some(PROFILES[0]))
    },
    option("--location", Some("gold")),
    flag("--yes"),
    flag(DRY_RUN),
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

const NEW_MODEL: [Argument; 8] = [
    positional("name", true),
    flag("--sql"),
    flag("--py"),
    option("--gold", None),
    flag(DRY_RUN),
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

/// The value is read from standard input, never taken as an argument (MG-REQ-1007).
const SECRET_SET: [Argument; 5] = [
    positional("name", true),
    flag(DRY_RUN),
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

const SCHEMA: [Argument; 5] = [
    Argument {
        values: &NAMES,
        ..positional("command", false)
    },
    flag("--all"),
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

impl Command {
    /// The name of the command, as typed, its words separated by one space.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::AddSource => NAMES[0],
            Self::Doctor => NAMES[1],
            Self::Explain => NAMES[2],
            Self::Init => NAMES[3],
            Self::Ls => NAMES[4],
            Self::NewModel => NAMES[5],
            Self::Plan => NAMES[6],
            Self::Schema => NAMES[7],
            Self::SecretSet => NAMES[8],
            Self::Validate => NAMES[9],
        }
    }

    /// What the command does, in one sentence.
    pub(crate) fn summary(self) -> &'static str {
        match self {
            Self::AddSource => "Create a source file that references its connection as a secret.",
            Self::Doctor => "Show where each secret reference resolves, never its value.",
            Self::Explain => "Explain an error code, or a model and its effective options.",
            Self::Init => "Create a project in the current folder.",
            Self::Ls => "List the selected models and why each one is selected.",
            Self::NewModel => "Create a model file with a minimal example.",
            Self::Plan => "Write the plan of the selected models; execute nothing.",
            Self::Schema => "Print the description of a command, or of every command, in JSON.",
            Self::SecretSet => "Store a secret read from standard input in the keychain.",
            Self::Validate => "Check the layout and the configuration of the project.",
        }
    }

    /// Every argument the command accepts, positional arguments first.
    pub(crate) fn arguments(self) -> &'static [Argument] {
        match self {
            Self::AddSource => &ADD_SOURCE,
            Self::Doctor => &GLOBAL,
            Self::Explain => &EXPLAIN,
            Self::Init => &INIT,
            Self::Ls => &LS,
            Self::NewModel => &NEW_MODEL,
            Self::Plan => &PLAN,
            Self::Schema => &SCHEMA,
            Self::SecretSet => &SECRET_SET,
            Self::Validate => &GLOBAL,
        }
    }

    /// The exit codes the command may end with.
    pub(crate) fn exit_codes(self) -> &'static [u8] {
        &EXIT_CODES
    }

    /// Whether `--json` makes the command print events; `schema` prints its document alone.
    pub(crate) fn json(self) -> bool {
        self != Self::Schema
    }

    /// Whether the command accepts `--dry-run`.
    pub(crate) fn dry_run(self) -> bool {
        self.arguments()
            .iter()
            .any(|argument| argument.name == DRY_RUN)
    }
}

/// The command named `name`, its words separated by one space.
pub(crate) fn lookup(name: &str) -> Option<Command> {
    ALL.into_iter().find(|command| command.name() == name)
}
