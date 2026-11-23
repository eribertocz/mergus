//! The output mode and whether it uses colors (MG-REQ-1321, MG-REQ-1347).

use mergus_ports::Environment;

/// How a run reports what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Standard output is a terminal: human output with colors.
    Interactive,
    /// No terminal, or a `dumb` one: human output without colors or animation.
    Plain,
    /// `--json`: events only on standard output.
    Events,
}

/// How much human output a run shows (MG-REQ-1347.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Verbosity {
    /// Only errors.
    Quiet,
    /// The default.
    #[default]
    Normal,
    /// Also one line per stage with its duration.
    Verbose,
}

/// Chooses the mode: interactive only when standard output is a `terminal`, `json` is false
/// and `TERM` is not `dumb`; otherwise plain, or events with `json` (MG-REQ-1347.1).
pub(crate) fn detect(json: bool, terminal: bool, environment: &dyn Environment) -> Mode {
    if json {
        Mode::Events
    } else if terminal && environment.var("TERM").as_deref() != Some("dumb") {
        Mode::Interactive
    } else {
        Mode::Plain
    }
}

/// Returns whether `mode` shows colors: only the interactive mode does, unless `NO_COLOR`
/// is set and not empty (MG-REQ-1347.2).
pub(crate) fn colors(mode: Mode, environment: &dyn Environment) -> bool {
    mode == Mode::Interactive
        && environment
            .var("NO_COLOR")
            .is_none_or(|value| value.is_empty())
}
