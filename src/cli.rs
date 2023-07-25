use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::types::SourceType;

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// Don't do anything; just show what would have happened
    #[arg(long)]
    pub dry_run: bool,

    /// The path of the GnuPG binary.
    ///
    /// This tool shells out to GnuPG. You can use this to override the path of the GnuPG command.
    #[arg(long, value_name = "PATH", default_value = "gpg")]
    pub gpg_path: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct KeySourceArgs {
    /// The public signing key for the repo
    ///
    /// This accepts the URL or local file path of a PGP key, in either binary or armored format.
    /// The key is downloaded and installed to /etc/apt/keyrings unless you pass --key-path.
    ///
    /// If you pass --keyserver, this is the key fingerprint.
    #[arg(short, long)]
    pub key: Option<String>,

    /// Do not install the public signing key for the repo
    ///
    /// Instead, all keys in the trusted keyrings will be considered valid signers for the
    /// repository, which is less secure and not recommended.
    #[arg(long)]
    pub force_no_key: bool,
}

#[derive(Args)]
#[group(required = false, multiple = false)]
pub struct KeyDestinationArgs {
    /// The path to install the repository signing key to
    ///
    /// By default, it's installed under /etc/apt/keyrings.
    #[arg(long, value_name = "PATH")]
    pub key_path: Option<PathBuf>,

    /// Inline the repository signing key into the source entry instead of installing it to a
    /// separate file
    #[arg(short, long)]
    pub inline_key: bool,
}

#[derive(Args)]
pub struct SigningKeyArgs {
    #[command(flatten)]
    pub location: KeySourceArgs,

    /// Download the repository signing key from this keyserver
    ///
    /// If this option is passed, --key is interpreted as the key fingerprint.
    #[arg(long, value_name = "URL")]
    pub keyserver: Option<String>,

    #[command(flatten)]
    pub destination: KeyDestinationArgs,
}

#[derive(Args)]
pub struct DescriptionArgs {
    /// A human-readable name for the source entry
    #[arg(short, long)]
    pub description: Option<String>,
}

#[derive(Args)]
pub struct DisabledArgs {
    /// Mark this source entry as disabled
    #[arg(long)]
    pub disabled: bool,
}

#[derive(Args)]
#[group(required = false, multiple = false)]
pub struct OverwriteArgs {
    /// Overwrite the source file if it already exists.
    #[arg(long)]
    pub overwrite: bool,

    /// Append a new entry to the source file if it already exists.
    #[arg(short, long)]
    pub append: bool,
}

#[derive(Args)]
pub struct New {
    /// The name of the source file
    ///
    /// The source file is generated in /etc/apt/sources.list.d/ with this as its basename.
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

    /// Add an additional option to the source entry
    ///
    /// Add an option that doesn't have its own flag in this CLI. See the sources.list(5) man page
    /// for a list of valid options.
    ///
    /// Options take the form `key=value`, or `key=value1,value2` to pass multiple values.
    #[arg(short, long, value_name = "KEY=VALUE")]
    pub option: Vec<String>,

    /// Allow invalid options names with --option
    ///
    /// Options passed with --option are added to the source entry literally, without checking if
    /// they're valid.
    #[arg(long)]
    pub force_literal_options: bool,

    #[command(flatten)]
    pub disabled: DisabledArgs,

    #[command(flatten)]
    pub overwrite: OverwriteArgs,
}

#[derive(Args)]
pub struct Add {
    /// The one-line-style source entry
    pub line: String,

    /// The name of the source file
    ///
    /// The source file is generated in /etc/apt/sources.list.d/ with this as its basename.
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
pub struct ConvertDestArgs {}

#[derive(Args)]
#[group(required = false, multiple = false)]
pub struct BackupArgs {}

#[derive(Args)]
pub struct Convert {
    /// The name of the source file
    ///
    /// This looks for a file in /etc/apt/sources.list.d/ with this basename and replaces it,
    /// deleting the original.
    #[arg(short, long)]
    pub name: Option<String>,

    /// The path of the single-line-style file to convert
    ///
    /// You must use this with --out. Unlike with --name, this file is not deleted.
    ///
    /// This can be `-` to read from stdin.
    #[arg(
        long = "in",
        value_name = "PATH",
        conflicts_with = "name",
        conflicts_with = "backup",
        conflicts_with = "backup_to",
        requires = "out_path"
    )]
    pub in_path: Option<PathBuf>,

    /// The path of the deb822 file to generate
    ///
    /// You must use this with --in.
    ///
    /// This can be `-` to write to stdout.
    #[arg(
        long = "out",
        value_name = "PATH",
        conflicts_with = "name",
        conflicts_with = "backup",
        conflicts_with = "backup_to",
        requires = "in_path"
    )]
    pub out_path: Option<PathBuf>,

    /// Backup the original `.list` file to `.list.bak` before replacing it
    #[arg(long, requires = "name", conflicts_with = "backup_to")]
    pub backup: bool,

    /// Backup the original `.list` file to this path before replacing it
    #[arg(
        long,
        value_name = "PATH",
        requires = "name",
        conflicts_with = "backup"
    )]
    pub backup_to: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add a new source entry by specifying its parameters
    New(New),

    /// Add a new source entry using the one-line syntax
    ///
    /// This parses the one-line-style entry and converts it to the more modern deb822 format before
    /// adding it to your repository sources.
    ///
    /// One-line-style source entries typically have this format:
    ///
    /// deb [ option1=value1 option2=value2 ] uri suite [component1] [component2] [...]
    Add(Add),

    /// Convert a single-line-style `.list` file to a deb822 `.sources` file
    ///
    /// You must pass either --name or both --in and --out.
    Convert(Convert),
}
