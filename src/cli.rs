use clap::{Args, Parser, Subcommand};

use crate::source::SourceType;

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct KeyLocationArgs {
    /// The public signing key for the repo
    ///
    /// This accepts the URL or local file path of a PGP key, in either GPG or armored format. The
    /// key is downloaded and installed to the directory specified by --keyring-dir.
    ///
    /// If you pass --keyserver, this is the key fingerprint.
    #[arg(short, long)]
    pub key: Option<String>,

    /// Do not install the public signing key for the repo
    ///
    /// Instead, all keys in the trusted keyrings will be considered valid signers for the
    /// repository, which is less secure.
    #[arg(long)]
    pub force_no_key: bool,
}

#[derive(Args)]
pub struct SigningKeyArgs {
    #[command(flatten)]
    pub location: KeyLocationArgs,

    /// Download the repository signing key from this keyserver
    ///
    /// If this option is passed, --key is interpreted as the key fingerprint.
    #[arg(long, value_name = "URL")]
    pub keyserver: Option<String>,

    /// The directory to install the repository signing key to
    #[arg(long, value_name = "PATH", default_value = "/etc/apt/keyrings")]
    pub keyring_dir: String,
}

#[derive(Args)]
pub struct DescriptionArgs {
    /// A human-readable name for the source
    #[arg(short, long)]
    pub description: Option<String>,
}

#[derive(Args)]
pub struct DisabledArgs {
    /// Mark this source as disabled
    #[arg(long)]
    pub disabled: bool,
}

#[derive(Args)]
pub struct OverwriteArgs {
    /// Overwrite the source file if it already exists.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Args)]
pub struct AddNew {
    /// A unique name for the source
    pub name: String,

    /// The URIs of the repository
    #[arg(long, required = true)]
    pub uri: Vec<String>,

    #[command(flatten)]
    pub description: DescriptionArgs,

    /// The repository suites (defaults to current distro version codename)
    #[arg(short, long)]
    pub suite: Vec<String>,

    /// The repository components
    #[arg(short, long, default_value = "main")]
    pub component: Vec<String>,

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
    pub key: SigningKeyArgs,

    /// The architectures to include
    #[arg(long)]
    pub arch: Vec<String>,

    /// The languages to include
    #[arg(long)]
    pub lang: Vec<String>,

    /// Add an additional option to the source file
    ///
    /// Add an option that doesn't have its own flag in this CLI. See the sources.list(5) man page
    /// for a list of valid options.
    ///
    /// Options take the form `key=value`, or `key=value1,value2` to pass multiple values.
    #[arg(short, long, value_name = "KEY=VALUE")]
    pub option: Vec<String>,

    /// Allow invalid options names with --option
    ///
    /// Options passed with --option are added to the source file literally, without checking if
    /// they're valid.
    #[arg(long)]
    pub force_literal_options: bool,

    #[command(flatten)]
    pub disabled: DisabledArgs,

    #[command(flatten)]
    pub overwrite: OverwriteArgs,
}

#[derive(Args)]
pub struct AddLine {
    /// The one-line-style source entry
    pub line: String,

    /// A unique name for the source
    #[arg(short, long)]
    pub name: String,

    #[command(flatten)]
    pub description: DescriptionArgs,

    #[command(flatten)]
    pub key: SigningKeyArgs,

    #[command(flatten)]
    pub disabled: DisabledArgs,

    #[command(flatten)]
    pub overwrite: OverwriteArgs,
}

#[derive(Args)]
pub struct AddPpa {
    /// The name of the PPA
    pub ppa: String,
}

#[derive(Args)]
pub struct Add {
    #[command(subcommand)]
    pub command: AddCommands,
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
    /// One-line-style source entries typically have this format:
    ///
    /// deb [ option1=value1 option2=value2 ] uri suite [component1] [component2] [...]
    Line(AddLine),

    /// Add a source from a PPA
    Ppa(AddPpa),
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add a new repository source
    Add(Add),
}
