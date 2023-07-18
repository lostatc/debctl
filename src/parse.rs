use std::str::FromStr;

use eyre::bail;
use pest::Parser;
use pest_derive::Parser;

use crate::error::Error;
use crate::option::{KnownOptionName, OptionMap, OptionValue};

#[derive(Parser)]
#[grammar = "line.pest"]
pub struct LineEntryParser;

fn parse_option_name(option_name: &str) -> eyre::Result<KnownOptionName> {
    match KnownOptionName::from_str(option_name) {
        Ok(option_name) => Ok(option_name),
        Err(_) => bail!(Error::MalformedSingleLineEntry {
            reason: format!(
                "\
                This is not a valid option name: `{option_name}`

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
                let source_type = rule.to_string();

                option_map.insert(KnownOptionName::Types, source_type);
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

                    let option_value: OptionValue = match option_values.as_slice() {
                        &["yes"] => true.into(),
                        &["no"] => false.into(),
                        &[value] => value.to_string().into(),
                        values => values
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .into(),
                    };

                    option_map.insert(option_name, option_value);
                }
            }
            Rule::param => {
                params.push(rule.as_str());
            }
            _ => unreachable!(),
        }
    }

    if let &[uri, suite, ref components @ ..] = params.as_slice() {
        let components = components
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        option_map.insert(KnownOptionName::Uris, uri.to_string());
        option_map.insert(KnownOptionName::Suites, suite.to_string());
        option_map.insert(KnownOptionName::Components, components);
    } else {
        unreachable!()
    }

    Ok(option_map)
}
