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

/// The exit codes every command may end with: success, a project error, an internal error.
const EXIT_CODES: [u8; 3] = [0, 1, 10];

/// A command the binary runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// `mergus add source` (MG-REQ-1304).
    AddSource,
    /// `mergus init` (MG-REQ-1301).
    Init,
    /// `mergus new model` (MG-REQ-1304).
    NewModel,
    /// `mergus schema` (MG-REQ-0263).
    Schema,
    /// `mergus validate` (MG-REQ-1345).
    Validate,
}

/// Every command, sorted by name (MG-REQ-1348).
pub(crate) const ALL: [Command; 5] = [
    Command::AddSource,
    Command::Init,
    Command::NewModel,
    Command::Schema,
    Command::Validate,
];

/// The names of every command, sorted.
pub(crate) const NAMES: [&str; 5] = ["add source", "init", "new model", "schema", "validate"];

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
            Self::Init => NAMES[1],
            Self::NewModel => NAMES[2],
            Self::Schema => NAMES[3],
            Self::Validate => NAMES[4],
        }
    }

    /// What the command does, in one sentence.
    pub(crate) fn summary(self) -> &'static str {
        match self {
            Self::AddSource => "Create a source file that references its connection as a secret.",
            Self::Init => "Create a project in the current folder.",
            Self::NewModel => "Create a model file with a minimal example.",
            Self::Schema => "Print the description of a command, or of every command, in JSON.",
            Self::Validate => "Check the layout and the configuration of the project.",
        }
    }

    /// Every argument the command accepts, positional arguments first.
    pub(crate) fn arguments(self) -> &'static [Argument] {
        match self {
            Self::AddSource => &ADD_SOURCE,
            Self::Init => &INIT,
            Self::NewModel => &NEW_MODEL,
            Self::Schema => &SCHEMA,
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
