use std::str::FromStr;

use eyre::bail;
use pest::Parser;
use pest_derive::Parser;

use crate::error::Error;
use crate::option::{KnownOptionName, OptionMap};

#[derive(Parser)]
#[grammar = "line.pest"]
pub struct LineEntryParser;

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
