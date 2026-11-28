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

/// The option that names the source whose indexes `doctor` checks (MG-REQ-0431).
pub(crate) const SOURCE: &str = "--source";

/// The option that makes a command that changes state idempotent (MG-REQ-1313).
pub(crate) const IDEMPOTENCY_KEY: &str = "--idempotency-key";

/// The run limits of MG-REQ-0710.1, whose values wait for #303.
pub(crate) const LIMITS: [&str; 3] = [
    "--max-wall-time",
    "--max-rows-written",
    "--max-bytes-scanned",
];

/// The first and last date of a backfill (MG-REQ-0657).
pub(crate) const FROM: &str = "--from";

/// The last date of a backfill; the run's start without it (MG-REQ-0657.1).
pub(crate) const TO: &str = "--to";

/// The model a backfill recomputes (MG-REQ-0657).
pub(crate) const MODEL: &str = "model";

/// The positional words of a selection (MG-REQ-1305).
pub(crate) const SELECTION: &str = "selection";

/// The exit codes every command may end with: success, a project error, an internal error.
const EXIT_CODES: [u8; 3] = [0, 1, 10];

/// The exit codes of `run` and `backfill`: those of every command, a failing step's code and
/// 7 when cancelled (MG-REQ-0710.2).
const RUN_EXIT_CODES: [u8; 5] = [0, 1, 6, 7, 10];

/// A command the binary runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// `mergus add source` (MG-REQ-1304).
    AddSource,
    /// `mergus backfill` (MG-REQ-0710, MG-REQ-0657).
    Backfill,
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
    /// `mergus run` (MG-REQ-0710).
    Run,
    /// `mergus schema` (MG-REQ-0263).
    Schema,
    /// `mergus secret set` (MG-REQ-1007).
    SecretSet,
    /// `mergus validate` (MG-REQ-1345).
    Validate,
}

/// Every command, sorted by name (MG-REQ-1348).
pub(crate) const ALL: [Command; 12] = [
    Command::AddSource,
    Command::Backfill,
    Command::Doctor,
    Command::Explain,
    Command::Init,
    Command::Ls,
    Command::NewModel,
    Command::Plan,
    Command::Run,
    Command::Schema,
    Command::SecretSet,
    Command::Validate,
];

/// The names of every command, sorted.
pub(crate) const NAMES: [&str; 12] = [
    "add source",
    "backfill",
    "doctor",
    "explain",
    "init",
    "ls",
    "new model",
    "plan",
    "run",
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

/// The idempotency key of a command that changes state (MG-REQ-1313).
const KEY: Argument = option(IDEMPOTENCY_KEY, None);

const ADD_SOURCE: [Argument; 6] = [
    positional("name", true),
    flag(DRY_RUN),
    KEY,
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

const DOCTOR: [Argument; 4] = [option(SOURCE, None), GLOBAL[0], GLOBAL[1], GLOBAL[2]];

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

const PARALLEL: Argument = Argument {
    r#type: Type::Integer,
    ..option(MAX_PARALLEL, Some("4"))
};

const PLAN: [Argument; 6] = [
    SELECTED, EXCLUDED, PARALLEL, GLOBAL[0], GLOBAL[1], GLOBAL[2],
];

/// What `run` and `backfill` share with `plan`, then the limits and the key (MG-REQ-0710.1).
const RUN: [Argument; 10] = [
    SELECTED,
    EXCLUDED,
    PARALLEL,
    option(LIMITS[0], None),
    option(LIMITS[1], None),
    option(LIMITS[2], None),
    KEY,
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

const BACKFILL: [Argument; 11] = [
    positional(MODEL, true),
    Argument {
        required: true,
        ..option(FROM, None)
    },
    option(TO, None),
    PARALLEL,
    option(LIMITS[0], None),
    option(LIMITS[1], None),
    option(LIMITS[2], None),
    KEY,
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

const INIT: [Argument; 9] = [
    option("--name", None),
    Argument {
        values: &PROFILES,
        ..option("--profile", Some(PROFILES[0]))
    },
    option("--location", Some("gold")),
    flag("--yes"),
    flag(DRY_RUN),
    KEY,
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

const NEW_MODEL: [Argument; 9] = [
    positional("name", true),
    flag("--sql"),
    flag("--py"),
    option("--gold", None),
    flag(DRY_RUN),
    KEY,
    GLOBAL[0],
    GLOBAL[1],
    GLOBAL[2],
];

/// The value is read from standard input, never taken as an argument (MG-REQ-1007).
const SECRET_SET: [Argument; 6] = [
    positional("name", true),
    flag(DRY_RUN),
    KEY,
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
        let index = ALL.iter().position(|command| *command == self);
        index
            .and_then(|index| NAMES.get(index))
            .copied()
            .unwrap_or_default()
    }

    /// What the command does, in one sentence.
    pub(crate) fn summary(self) -> &'static str {
        match self {
            Self::AddSource => "Create a source file that references its connection as a secret.",
            Self::Backfill => "Recompute the periods of a model in a date range into work.",
            Self::Doctor => {
                "Show where each secret reference resolves, never its value; with `--source`, check the indexes of that source."
            }
            Self::Explain => "Explain an error code, or a model and its effective options.",
            Self::Init => "Create a project in the current folder.",
            Self::Ls => "List the selected models and why each one is selected.",
            Self::NewModel => "Create a model file with a minimal example.",
            Self::Plan => "Write the plan of the selected models; execute nothing.",
            Self::Run => "Plan and execute the selected models into work; publish nothing.",
            Self::Schema => "Print the description of a command, or of every command, in JSON.",
            Self::SecretSet => "Store a secret read from standard input in the keychain.",
            Self::Validate => "Check the layout and the configuration of the project.",
        }
    }

    /// Every argument the command accepts, positional arguments first.
    pub(crate) fn arguments(self) -> &'static [Argument] {
        match self {
            Self::AddSource => &ADD_SOURCE,
            Self::Backfill => &BACKFILL,
            Self::Doctor => &DOCTOR,
            Self::Explain => &EXPLAIN,
            Self::Init => &INIT,
            Self::Ls => &LS,
            Self::NewModel => &NEW_MODEL,
            Self::Plan => &PLAN,
            Self::Run => &RUN,
            Self::Schema => &SCHEMA,
            Self::SecretSet => &SECRET_SET,
            Self::Validate => &GLOBAL,
        }
    }

    /// The exit codes the command may end with.
    pub(crate) fn exit_codes(self) -> &'static [u8] {
        match self {
            Self::Run | Self::Backfill => &RUN_EXIT_CODES,
            _ => &EXIT_CODES,
        }
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
