use std::io::{BufRead, BufReader, Read};
use std::str::FromStr;

use eyre::{bail, WrapErr};
use pest::Parser;
use pest_derive::Parser;

use crate::error::Error;
use crate::option::{KnownOptionName, OptionMap, OptionName, OptionPair, OptionValue};

#[derive(Parser)]
#[grammar = "line.pest"]
pub struct LineEntryParser;

/// Parse a custom option in `key=value` format.
pub fn parse_custom_option(option: &str, force_literal: bool) -> eyre::Result<OptionPair> {
    let (key, value) = match option.trim().split_once('=') {
        Some(pair) => pair,
        None => bail!(Error::MalformedOption {
            option: option.to_string()
        }),
    };

    let option_name = if force_literal {
        OptionName::Custom(key.to_string())
    } else {
        KnownOptionName::from_str(key)?.into()
    };

    let option_value: OptionValue = if value.contains(',') {
        value
            .split(',')
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .into()
    } else {
        value.to_string().into()
    };

    Ok((option_name, option_value))
}

/// Parse a known option name, returning an error if it's not recognized.
fn parse_option_name(option_name: &str) -> eyre::Result<KnownOptionName> {
    match KnownOptionName::from_str(option_name) {
        Ok(option_name) => Ok(option_name),
        Err(_) => bail!(Error::MalformedOneLineEntry {
            reason: format!(
                "\
                This is not a valid option name: `{option_name}`.\n\n\
                See the sources.list(5) man page for a list of valid options.
                "
            ),
        }),
    }
}

/// Parse a one-line-style source entry.
pub fn parse_line_entry(entry: &str) -> eyre::Result<OptionMap> {
    let line = match LineEntryParser::parse(Rule::line, entry) {
        Ok(mut result) => result.next().unwrap(),
        Err(err) => bail!(Error::MalformedOneLineEntry {
            reason: err.to_string()
        }),
    };

    let mut option_map = OptionMap::new();
    let mut params = Vec::new();

    for rule in line.into_inner() {
        match rule.as_rule() {
            Rule::source_type => {
                option_map.insert(KnownOptionName::Types, rule.as_str());
            }
            Rule::option_list => {
                for option in rule.into_inner() {
                    let mut option_rules = option.into_inner();

                    let option_name = parse_option_name(option_rules.next().unwrap().as_str())?;
                    let value_list = option_rules.next().unwrap();

                    let option_values = value_list
                        .into_inner()
                        .map(|rule| rule.as_str())
                        .collect::<Vec<_>>();

                    option_map.insert(option_name, option_values);
                }
            }
            Rule::param => {
                params.push(rule.as_str());
            }
            Rule::EOI => {}
            _ => unreachable!("unexpected parsing rule: {:?}", rule.as_rule()),
        }
    }

    if let &[uri, suite, ref components @ ..] = params.as_slice() {
        option_map.insert(KnownOptionName::Uris, uri);
        option_map.insert(KnownOptionName::Suites, suite);
        option_map.insert(KnownOptionName::Components, components.to_vec());
    } else {
        unreachable!("failed parsing uri, suite, and components")
    }

    Ok(option_map)
}

/// A line in a one-line-style source file.
#[derive(Debug, Clone)]
pub enum ConvertedLineEntry {
    Entry(OptionMap),
    Comment(String),
}

/// The character used to comment out lines in a one-line-style source file.
const COMMENT_CHAR: char = '#';

/// Options for parsing a one-line-style source file.
#[derive(Debug, Clone)]
pub struct ParseLineFileOptions {
    pub skip_comments: bool,
    pub skip_disabled: bool,
}

/// Parse a file of one-line-style source entries.
///
/// Comments are preserved unless `skip_comments` is true. Entries that are commented out are
/// converted to disabled entries in the output unless `skip_disabled` is true.
///
pub fn parse_line_file(
    mut file: impl Read,
    options: &ParseLineFileOptions,
) -> eyre::Result<Vec<ConvertedLineEntry>> {
    let mut entry_list = Vec::new();

    for line_result in BufReader::new(&mut file).lines() {
        let line = line_result.wrap_err("failed reading source file")?;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        let entry = if trimmed.starts_with(COMMENT_CHAR) {
            // This line is a comment. Get the part after the first comment char.
            let disabled_line = match line.split_once(COMMENT_CHAR) {
                Some((_, disabled_line)) => disabled_line.trim(),
                None => bail!("failed splitting line on comment character"),
            };

            // Check if the part after the first comment char is a valid line entry. If it is, we
            // create a new disabled entry in the converted output file..
            match parse_line_entry(disabled_line) {
                Ok(mut option_map) => {
                    if options.skip_disabled {
                        continue;
                    }

                    // Disable this entry.
                    option_map.insert(KnownOptionName::Enabled, false);

                    ConvertedLineEntry::Entry(option_map)
                }
                Err(err) => match err.downcast_ref::<Error>() {
                    // Don't fail on a malformed line entry here. If the part after the first
                    // comment char isn't a valid line entry, that just means it's a normal comment.
                    Some(Error::MalformedOneLineEntry { .. }) => {
                        if options.skip_comments {
                            continue;
                        }

                        ConvertedLineEntry::Comment(disabled_line.to_string())
                    }
                    _ => {
                        bail!(err.wrap_err("failed parsing disabled one-line-style source entry"))
                    }
                },
            }
        } else {
            // This is a normal not-commented-out line entry.
            let mut option_map =
                parse_line_entry(&line).wrap_err("failed parsing one-line-style source entry")?;

            // We always include the `Enabled` option, even for non-disabled entries. It makes
            // disabling them manually as a user easier.
            option_map.insert(KnownOptionName::Enabled, true);

            ConvertedLineEntry::Entry(option_map)
        };

        entry_list.push(entry);
    }

    Ok(entry_list)
}
