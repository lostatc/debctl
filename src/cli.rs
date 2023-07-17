use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SourceType {
    /// A binary package
    Deb,

    /// A source package
    DebSrc,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct KeyArgs {
    /// The URL of the public signing key to download
    ///
    /// This can be armored or GPG format.
    #[arg(long, value_name = "URL")]
    key_url: Option<String>,

    /// The fingerprint of the public signing key to fetch from the keyserver
    #[arg(long, value_name = "HASH")]
    fingerprint: Option<String>,

    /// Mark this source as trusted, disabling signature verification (dangerous)
    #[arg(long)]
    trusted: bool,
}

#[derive(Args)]
pub struct AddNew {
    /// A unique name to give the source
    pub name: String,

    /// The URI of the repository
    #[arg(long)]
    pub uri: String,

    /// A human-readable description of the source
    #[arg(short, long)]
    pub description: Option<String>,

    /// The repository suites (defaults to current distro version codename)
    #[arg(short, long)]
    pub suites: Vec<String>,

    /// The repository components
    #[arg(short, long, default_value = "main")]
    pub components: Vec<String>,

    /// The source types to include
    #[arg(
        id = "type",
        value_name = "TYPE",
        short,
        long,
        value_enum,
        default_value = "deb"
    )]
    pub kind: Vec<SourceType>,

    #[command(flatten)]
    pub key: KeyArgs,

    /// The keyserver to fetch the public signing key from
    #[arg(long, value_name = "URL", default_value = "keyserver.ubuntu.com")]
    pub keyserver: Option<String>,

    /// The architectures to include
    #[arg(long)]
    pub arch: Vec<String>,

    /// The languages to include
    #[arg(long)]
    pub lang: Vec<String>,

    /// The download targets (uncommon)
    #[arg(long)]
    pub targets: Vec<String>,

    /// Use PDiffs to update old indexes (uncommon)
    #[arg(long)]
    pub pdiffs: bool,

    /// Acquire indexes via a URI constructed from a hashsum (uncommon)
    #[arg(long)]
    pub by_hash: bool,

    /// Mark this source as disabled
    #[arg(long)]
    pub disable: bool,
}

#[derive(Args)]
pub struct AddLine {
    /// The one-line-style source entry
    line: String,
}

#[derive(Args)]
pub struct AddPpa {
    /// The name of the PPA
    ppa: String,
}

#[derive(Args)]
pub struct Add {
    #[command(subcommand)]
    command: AddCommands,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum AddCommands {
    /// Add a source by specifying its parameters
    New(AddNew),

    /// Add a source by its one-line-style source entry
    ///
    /// This parses the one-line-style entry and converts it to the more modern deb822 format before
    /// adding it to your repository sources.
    ///
    /// An example of a one-line-style source entry:
    ///
    /// deb http://deb.debian.org/debian bookworm main
    Line(AddLine),

    /// Add a source from a PPA
    Ppa(AddPpa),
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add a new repository source
    Add(Add),
}
