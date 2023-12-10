use secrecy::Secret;
// use serde_aux::field_attributes::deserialize_number_from_string;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Settings {
    pub basics: BasicSettings,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct BasicSettings {
    pub imapserver: String,
    pub login: String,
    pub password: Secret<String>,
    // A matcher set is a list of one or more matchers, all of which must fit a given mail for it
    // to be kept.  Currently just a vec, but we're likely to add flags later.
    pub matcher_sets: Vec<Vec<Matcher>>,
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
}

// This isn't *really* a test, it's an exploration tool for
// testing TOML serialization; make the TestConfig struct you
// want and output it here, so you can see what it looks like
// in TOML.
//
// Note that this is currently the only place the `toml` crate is used
//
// Must be run with `cargo test -- --nocapture` to be of any use
#[cfg(test)]
mod toml_test {
    use crate::configuration::Matcher;
    use crate::configuration::MatcherPart;
    use regex::Regex;

    #[test]
    fn test_toml_output() {
        #[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
        pub struct TestConfig {
            matchers: Vec<Matcher>,
        }

        let test = TestConfig {
            matchers: vec![
                Matcher::Match(MatcherPart::Subject(Regex::new("aou").unwrap())),
                Matcher::Match(MatcherPart::From(Regex::new("stn").unwrap())),
            ],
        };
        println!("toml test: {:?}", test);
        let toml = toml::to_string(&test);
        match toml {
            Ok(val) => println!("{}", val),
            Err(e) => println!("err: {:?}", e),
        }
    }
}

// #[tracing::instrument]
// pub fn make_checker(matches: &[(MailPart, &str)], unmatches: &[(MailPart, &str)]) -> Checker {
//     Checker {
//         matches: matches
//             .iter()
//             .map(|(x, y)| Match {
//                 part: x.clone(),
//                 regex: Regex::new(y).unwrap(),
//             })
//             .collect(),
//         unmatches: unmatches
//             .iter()
//             .map(|(x, y)| Match {
//                 part: x.clone(),
//                 regex: Regex::new(y).unwrap(),
//             })
//             .collect(),
//     }
// }

pub fn get_environment() -> Environment {
    // Detect the running environment.
    // Default to `prod` if unspecified.
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "prod".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");

    environment
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("settings");

    let environment = get_environment();
    let environment_filename = format!("{}.toml", environment.as_str());
    let settings = config::Config::builder()
        // .set_default("database.database_name", "newsletter")?
        // .add_source(config::File::from(
        //     configuration_directory.join("base.yaml"),
        // ))
        .add_source(config::File::from(
            configuration_directory.join(environment_filename),
        ))
        // Add in settings from environment variables (with a prefix of APP and '__' as separator)
        // E.g. `APP_APPLICATION__PORT=5001 would set `Settings.application.port`
        .add_source(
            config::Environment::with_prefix("APP")
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
