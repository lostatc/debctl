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
        Err(_) => bail!(Error::MalformedSingleLineEntry {
            reason: format!(
                "\
                This is not a valid option name: `{option_name}`.\n\n\
                See the sources.list(5) man page for a list of valid options.
                "
            ),
        }),
    }
}

/// Parse a single-line-style source entry.
pub fn parse_line_entry(entry: &str) -> eyre::Result<OptionMap> {
    let line = match LineEntryParser::parse(Rule::line, entry) {
        Ok(mut result) => result.next().unwrap(),
        Err(err) => bail!(Error::MalformedSingleLineEntry {
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

pub fn parse_line_file(mut file: impl Read) -> eyre::Result<Vec<OptionMap>> {
    let mut options_list = Vec::new();

    for line_result in BufReader::new(&mut file).lines() {
        let line = line_result.wrap_err("failed reading source file")?;

        if line.trim().starts_with('#') {
            // This line is a comment.
            continue;
        }

        let options =
            parse_line_entry(&line).wrap_err("failed parsing single-line-style source entry")?;

        options_list.push(options);
    }

    Ok(options_list)
}
