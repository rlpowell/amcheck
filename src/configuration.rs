use secrecy::Secret;
use tracing::debug;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Settings {
    pub imapserver: String,
    pub login: String,
    pub password: Secret<String>,
    // A matcher set is a list of one or more matchers, all of which must fit a given mail for it
    // to be kept.  Currently just a vec, but we're likely to add flags later.
    pub matcher_sets: Vec<Vec<Matcher>>,
    // A checker set is a list of one or more checks, which email must match for complaints to not
    // be produced.  Also used to clean things up. Example: "Must be at least one puppet run in the
    // past day" and "alert on failed puppet runs" and "delete successful puppet runs older than 2
    // days".
    pub checker_sets: Vec<Checker>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Matcher {
    Match(MatcherPart),
    UnMatch(MatcherPart),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum MatcherPart {
    #[serde(with = "serde_regex")]
    Subject(regex::Regex),
    #[serde(with = "serde_regex")]
    From(regex::Regex),
    #[serde(with = "serde_regex")]
    Body(regex::Regex),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Checker {
    pub name: String,
    pub matchers: Vec<Matcher>,
    pub dates: Vec<DateLimit>,
    pub checks: Vec<Check>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Check {
    CheckIndividual(CheckIndividual),
    CheckCount(CountLimit),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CheckIndividual {
    pub matchers: Vec<Matcher>,
    pub actions: CheckActions,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CheckActions {
    pub matched: CheckAction,
    pub unmatched: CheckAction,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum CheckAction {
    Alert,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum DateLimit {
    OlderThan(u8),
    YoungerThan(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum CountLimit {
    AtLeast(u8),
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
    use crate::configuration::Check::*;
    use crate::configuration::CheckAction::*;
    use crate::configuration::CheckActions;
    use crate::configuration::CheckIndividual;
    use crate::configuration::Checker;
    use crate::configuration::CountLimit::*;
    use crate::configuration::DateLimit::*;

    #[test]
    fn test_json_output() {
        #[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
        pub struct TestConfig {
            checkers: Vec<Checker>,
        }

        let test = TestConfig {
            checkers: vec![Checker {
                name: "foo".to_owned(),
                matchers: vec![],
                dates: vec![OlderThan(1)],
                checks: vec![
                    CheckCount(AtLeast(1)),
                    CheckIndividual(CheckIndividual {
                        matchers: vec![],
                        actions: CheckActions {
                            matched: Delete,
                            unmatched: Alert,
                        },
                    }),
                ],
            }],
        };
        println!("json test original:\n{:#?}", test);
        let json = serde_json::to_string_pretty(&test);
        match json {
            Ok(val) => println!("json test output:\n{}", val),
            Err(e) => println!("err: {:?}", e),
        }
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
