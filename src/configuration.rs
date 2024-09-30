use secrecy::Secret;
use tracing::debug;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Settings {
    pub imapserver: String,
    pub login: String,
    pub password: Secret<String>,
    pub handlers: Vec<Handler>,
}

// FIXME: put this in the readme
//
// Mail is handled in two passes.  In the first pass, any mail that matches all the `Filter`s on
// each Handler is moved from INBOX to amcheck_storage.  In the second pass, we match that same
// set again, and then we walk the `CheckerTree`, which will lead to various actions like deleting
// mails that we're happy with or alerting on mails we're not.
//
// Examples of the sorts of things we do with the check tree: "Must be at least one puppet run in
// the past day" and "alert on failed puppet runs" and "delete successful puppet runs older than 2
// days".
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Handler {
    pub name: String,
    pub filters: Vec<Filter>,
    pub checker_tree: CheckerTree,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum CheckerTree {
    Empty,
    Action(Action),
    MatchCheck(MatchCheck),
    DateCheck(DateCheck),
    CountCheck(CountCheck),
    BodyCheck(BodyCheck),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct MatchCheck {
    pub matchers: Vec<Filter>,
    pub matched: Box<CheckerTree>,
    pub not_matched: Box<CheckerTree>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct DateCheck {
    pub days: u8,
    pub older_than: Box<CheckerTree>,
    pub younger_than: Box<CheckerTree>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CountCheck {
    pub count: u8,
    pub greater_than: Box<CheckerTree>,
    pub less_than: Box<CheckerTree>,
    pub equal: Box<CheckerTree>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct BodyCheck {
    pub string: String,
    pub matched: Box<CheckerTree>,
    pub not_matched: Box<CheckerTree>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Action {
    Alert,
    Delete,
    Success,
    Nothing,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Filter {
    Match(MatcherPart),
    UnMatch(MatcherPart),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum MatcherPart {
    #[serde(with = "serde_regex")]
    Subject(regex::Regex),
    #[serde(with = "serde_regex")]
    From(regex::Regex),
}

// This isn't *really* a test, it's an exploration tool for
// testing JSON serialization; make the TestConfig struct you
// want and output it here, so you can see what it looks like
// in JSON.
//
// Note that this is currently the only place the `serde_json` crate is used
//
// Must be run with `cargo test -- --nocapture` to be of any use
#[cfg(test)]
mod json_test {
    use crate::configuration::Action::*;
    use crate::configuration::BodyCheck;
    use crate::configuration::CheckerTree::*;
    use crate::configuration::Filter::*;
    use crate::configuration::MatcherPart::*;
    use crate::configuration::{CountCheck, DateCheck, Handler, MatchCheck};

    #[test]
    fn test_json_output() {
        #[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
        pub struct TestConfig {
            ms: Handler,
        }

        let test = TestConfig {
            ms: Handler {
                name: "foo".to_owned(),
                filters: vec![Match(From(
                    regex::Regex::new("root@digitalkingdom.org").unwrap(),
                ))],
                checker_tree: MatchCheck(MatchCheck {
                    matchers: vec![],
                    matched: Box::new(BodyCheck(BodyCheck {
                        string: "Notice: Applied catalog in".to_string(),
                        matched: Box::new(DateCheck(DateCheck {
                            days: 1,
                            older_than: Box::new(Action(Delete)),
                            younger_than: Box::new(CountCheck(CountCheck {
                                count: 1,
                                equal: Box::new(Action(Success)),
                                less_than: Box::new(Action(Success)),
                                greater_than: Box::new(Action(Alert)),
                            })),
                        })),
                        not_matched: Box::new(Action(Alert)),
                    })),
                    not_matched: Box::new(Action(Alert)),
                }),
            },
        };
        println!("json test original:\n{:#?}", test);
        let json = serde_json::to_string_pretty(&test);
        match json {
            Ok(val) => println!("json test output:\n{}", val),
            Err(e) => println!("err: {:?}", e),
        }
    }

    #[test]
    fn test_json_input() {
        #[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
        pub struct TestConfig {
            ms: Handler,
        }

        let json = r#"
{
  "ms": {
    "name": "foo",
    "filters": [
      {
        "Match": {
          "From": "root@digitalkingdom.org"
        }
      }
    ],
    "checker_tree": {
      "MatchCheck": {
        "matchers": [
          {
            "Match": {
              "Body": "Notice: Applied catalog in"
            }
          }
        ],
        "matched": {
          "DateCheck": {
            "days": 1,
            "older_than": {
              "Action": "Delete"
            },
            "younger_than": {
              "CountCheck": {
                "count": 1,
                "greater_than": {
                  "Action": "Alert"
                },
                "less_than": {
                  "Action": "Success"
                },
                "equal": {
                  "Action": "Success"
                }
              }
            }
          }
        },
        "not_matched": {
          "Action": "Alert"
        }
      }
    }
  }
}
        "#;
        let output: TestConfig = serde_json::from_str(json).unwrap();
        println!("rust output: #{output:#?}");
    }
}

pub fn get_environment() -> Environment {
    // Detect the running environment.
    // Default to `prod` if unspecified.
    let environment: Environment = std::env::var("AMCHECK_ENVIRONMENT")
        .unwrap_or_else(|_| "prod".into())
        .try_into()
        .expect("Failed to parse AMCHECK_ENVIRONMENT.");

    environment
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("settings");
    let environment = get_environment();
    let environment_filename = format!("{}.json5", environment.as_str());

    let config_file: std::path::PathBuf = match std::env::var("AMCHECK_CONFIG_FILE") {
        Ok(name) => name.into(),
        Err(_) => configuration_directory.join(environment_filename),
    };

    debug!("Config file: {config_file:?}");

    let settings = config::Config::builder()
        // .set_default("database.database_name", "newsletter")?
        .add_source(config::File::from(config_file))
        // Add in settings from environment variables (with a prefix of AMCHECK and '__' as separator)
        // E.g. `AMCHECK_APPLICATION__PORT=5001 would set `Settings.application.port`
        .add_source(
            config::Environment::with_prefix("AMCHECK")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    settings.try_deserialize::<Settings>()
}

/// The possible runtime environment for our application.
#[derive(Clone, Debug, PartialEq)]
pub enum Environment {
    Test,
    Prod,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Test => "test",
            Environment::Prod => "prod",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "test" => Ok(Self::Test),
            "production" => Ok(Self::Prod),
            "prod" => Ok(Self::Prod),
            other => Err(format!(
                "{} is not a supported environment. Use either `test` or `prod`.",
                other
            )),
        }
    }
}
