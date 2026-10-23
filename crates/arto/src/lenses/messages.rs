//! The reader's words for what the lens engine reports.
//!
//! The engine's errors and placeholders are values with an English
//! `Display`, for the log; what the interface shows is worded here, from the
//! locale files, so that the engine does not reach for the app's
//! translations.

use super::agent::AgentError;
use super::job::{self, Placeholder};
use super::models::{self, ModelSource, ModelsError};
use super::runner::{stderr_suffix, RunError, MAX_OUTPUT};
use super::secrets::{self, SecretsError};
use super::session::{AnswerError, KeyError};
use arto_config::LensDisplay;
use rust_i18n::t;

/// A value the reader is shown, in the interface's language.
pub(crate) trait Translated {
    /// The value in `locale`.
    fn translated_in(&self, locale: &str) -> String;

    /// The value in the language the interface is shown in.
    fn translated(&self) -> String {
        self.translated_in(&rust_i18n::locale())
    }
}

impl Translated for RunError {
    fn translated_in(&self, locale: &str) -> String {
        match self {
            Self::NoCommand => t!("lenses.errors.no_command", locale = locale),
            Self::Spawn { program, source } => t!(
                "lenses.errors.spawn",
                locale = locale,
                program = program,
                reason = source
            ),
            Self::Io(source) => t!("lenses.errors.io", locale = locale, reason = source),
            Self::Timeout(timeout) => t!(
                "lenses.errors.timeout",
                locale = locale,
                seconds = timeout.as_secs()
            ),
            Self::TooLarge => t!(
                "lenses.errors.too_large",
                locale = locale,
                mib = MAX_OUTPUT / 1024 / 1024
            ),
            Self::Failed { status, stderr } => {
                let status = status.map_or_else(String::new, |code| {
                    t!("lenses.errors.with_status", locale = locale, code = code).into_owned()
                });
                t!(
                    "lenses.errors.failed",
                    locale = locale,
                    status = status,
                    stderr = stderr_suffix(stderr)
                )
            }
            Self::NotUtf8 => t!("lenses.errors.not_utf8", locale = locale),
            Self::Unreachable(reason) => {
                t!(
                    "lenses.errors.unreachable",
                    locale = locale,
                    reason = reason
                )
            }
            Self::Status { status, message } => t!(
                "lenses.errors.status",
                locale = locale,
                status = status,
                message = stderr_suffix(message)
            ),
            Self::ApiKey(reason) => t!(
                "lenses.errors.api_key",
                locale = locale,
                reason = reason.translated_in(locale)
            ),
        }
        .into_owned()
    }
}

impl Translated for KeyError {
    fn translated_in(&self, locale: &str) -> String {
        match self {
            Self::Empty => t!("lenses.errors.empty_key", locale = locale).into_owned(),
            Self::Command(error) => error.translated_in(locale),
            Self::Store(error) => error.translated_in(locale),
        }
    }
}

impl Translated for SecretsError {
    fn translated_in(&self, locale: &str) -> String {
        match self {
            Self::Unavailable(reason) => t!(
                "lenses.errors.credential_store",
                locale = locale,
                reason = reason
            )
            .into_owned(),
            Self::Store(_) | Self::Interrupted(_) => self.to_string(),
        }
    }
}

impl Translated for AnswerError {
    fn translated_in(&self, locale: &str) -> String {
        match self {
            Self::Run(error) => error.translated_in(locale),
            Self::Agent(error) => error.translated_in(locale),
        }
    }
}

/// The agent's own words, which are not the interface's to translate.
impl Translated for AgentError {
    fn translated_in(&self, _locale: &str) -> String {
        self.to_string()
    }
}

impl Translated for ModelsError {
    fn translated_in(&self, locale: &str) -> String {
        match self {
            Self::NoProgram => t!("lenses.errors.no_program", locale = locale).into_owned(),
            Self::Run(error) => error.translated_in(locale),
            Self::Listing(_) => self.to_string(),
        }
    }
}

impl Translated for Placeholder {
    fn translated_in(&self, locale: &str) -> String {
        match self {
            Self::Block => t!("lenses.message_shape.block", locale = locale),
            Self::Before => t!("lenses.message_shape.before", locale = locale),
            Self::After => t!("lenses.message_shape.after", locale = locale),
            Self::Whole => t!("lenses.message_shape.whole", locale = locale),
        }
        .into_owned()
    }
}

/// [`job::message_shape`], its placeholders in the interface's language.
pub(crate) fn message_shape(display: LensDisplay, prompt: Option<&str>) -> String {
    job::message_shape(display, prompt, |placeholder| placeholder.translated())
}

/// [`models::available_models`], or why not in the interface's language.
pub(crate) async fn available_models(source: &ModelSource) -> Result<Vec<String>, String> {
    models::available_models(source)
        .await
        .map_err(|error| error.translated())
}

/// [`secrets::is_stored`], or why not in the interface's language.
pub(crate) async fn is_stored(account: String) -> Result<bool, String> {
    secrets::is_stored(account)
        .await
        .map_err(|error| error.translated())
}

/// [`secrets::store_key`], or why not in the interface's language.
pub(crate) async fn store_key(account: String, key: String) -> Result<(), String> {
    secrets::store_key(account, key)
        .await
        .map_err(|error| error.translated())
}

/// [`secrets::forget_key`], or why not in the interface's language.
pub(crate) async fn forget_key(account: String) -> Result<(), String> {
    secrets::forget_key(account)
        .await
        .map_err(|error| error.translated())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn failed(status: Option<i32>, stderr: &str) -> RunError {
        RunError::Failed {
            status,
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn a_failed_command_says_its_status_and_what_it_wrote() {
        assert_eq!(
            failed(Some(2), "boom").translated_in("en"),
            "the command failed with status 2: boom"
        );
        assert_eq!(
            failed(Some(2), "boom").translated_in("ja"),
            "コマンドが失敗しました（ステータス 2）: boom"
        );
        assert_eq!(
            failed(None, "").translated_in("ja"),
            "コマンドが失敗しました"
        );
    }

    #[test]
    fn a_timeout_and_an_oversized_answer_name_their_limit() {
        assert_eq!(
            RunError::Timeout(Duration::from_secs(30)).translated_in("ja"),
            "コマンドが 30 秒以内に終わりませんでした"
        );
        assert_eq!(
            RunError::TooLarge.translated_in("en"),
            "the command wrote more than 16 MiB"
        );
    }

    #[test]
    fn a_server_status_carries_the_message_it_came_with() {
        let error = RunError::Status {
            status: 401,
            message: "Incorrect API key provided".to_string(),
        };

        assert_eq!(
            error.translated_in("ja"),
            "サーバーがステータス 401 を返しました: Incorrect API key provided"
        );
    }

    #[test]
    fn a_missing_key_says_why_inside_the_api_key_message() {
        assert_eq!(
            RunError::ApiKey(KeyError::Empty).translated_in("en"),
            "cannot get the API key: it is empty"
        );
        let command = KeyError::Command(Box::new(RunError::NoCommand));
        assert_eq!(
            RunError::ApiKey(command).translated_in("ja"),
            "API キーを取得できません: レンズにコマンドがありません"
        );
    }

    #[test]
    fn what_an_agent_said_is_shown_as_it_said_it() {
        let error = AnswerError::from(AgentError("Credit balance is too low".to_string()));

        assert_eq!(error.translated_in("ja"), "Credit balance is too low");
    }

    #[test]
    fn a_models_lookup_without_a_program_says_so() {
        assert_eq!(
            ModelsError::NoProgram.translated_in("ja"),
            "エージェントのプログラムがありません"
        );
    }

    #[test]
    fn the_english_log_line_reads_as_the_english_interface_does() {
        let errors = [
            RunError::NoCommand,
            failed(Some(1), "nope"),
            RunError::Unreachable("refused".to_string()),
            RunError::ApiKey(KeyError::Empty),
        ];
        for error in errors {
            assert_eq!(error.to_string(), error.translated_in("en"));
        }
    }

    #[test]
    fn the_placeholders_of_a_message_are_worded_in_the_locale() {
        let shape = job::message_shape(LensDisplay::Annotate, None, |placeholder| {
            placeholder.translated_in("ja")
        });

        assert_eq!(shape, "…ブロック…\n");
        assert_eq!(
            Placeholder::Whole.translated_in("en"),
            Placeholder::Whole.to_string()
        );
    }
}
