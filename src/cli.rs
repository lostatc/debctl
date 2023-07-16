use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SourceType {
    /// A binary package
    Deb,

    /// A source package
    DebSrc,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct KeyArgs {
    /// The URL of the public signing key to download
    ///
    /// This can be armored or GPG format.
    #[arg(long, value_name = "URL")]
    key_url: Option<String>,

    /// The fingerprint of the public signing key to fetch from the keyserver
    #[arg(long)]
    fingerprint: Option<String>,

    /// Mark this source as trusted, disabling signature verification (dangerous)
    #[arg(long)]
    trusted: bool,
}

#[derive(Args)]
struct AddNew {
    /// A unique name to give the source
    name: String,

    /// The URI of the repository
    #[arg(long)]
    uri: String,

    /// A human-readable description of the source
    #[arg(short, long)]
    description: Option<String>,

    /// The repository suites (defaults to current distro version codename)
    #[arg(short, long)]
    suites: Vec<String>,

    /// The repository components
    #[arg(short, long, default_value = "main")]
    components: Vec<String>,

    /// The type of source
    #[arg(
        id = "type",
        value_name = "TYPE",
        short,
        long,
        value_enum,
        default_value = "deb"
    )]
    kind: SourceType,

    #[command(flatten)]
    key: KeyArgs,

    /// The keyserver to fetch the public signing key from
    #[arg(long, default_value = "keyserver.ubuntu.com")]
    keyserver: Option<String>,

    /// The architectures to include
    #[arg(long)]
    arch: Vec<String>,

    /// The languages to include
    #[arg(long)]
    lang: Vec<String>,

    /// The download targets (uncommon)
    #[arg(long)]
    targets: Vec<String>,

    /// Use PDiffs to update old indexes (uncommon)
    #[arg(long)]
    pdiffs: bool,

    /// Acquire indexes via a URI constructed from a hashsum (uncommon)
    #[arg(long)]
    by_hash: bool,

    /// Make this source as disabled
    #[arg(long)]
    disable: bool,
}

#[derive(Args)]
struct AddLine {
    /// The one-line-style source directive
    line: String,
}

#[derive(Args)]
struct AddPpa {
    /// The name of the PPA
    ppa: String,
}

#[derive(Args)]
struct Add {
    #[command(subcommand)]
    command: AddCommands,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum AddCommands {
    /// Add a source by specifying its parameters
    New(AddNew),

    /// Add a source by its one-line-style source directive
    Line(AddLine),

    /// Add a source from a PPA
    Ppa(AddPpa),
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new repository source
    Add(Add),
}
