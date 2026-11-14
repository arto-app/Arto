//! The MCP tools, each one request of the socket protocol.

use crate::instance::Instance;
use crate::roots::Roots;
use arto_lsp::{
    AskError, Call, GotoParams, OpenEvent, OpenRequest, ScrollPosition, SelectionParams,
    StateParams, StateResult, WindowKey, WindowOptions,
};
use arto_types::FileOpenBehavior;
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const INSTRUCTIONS: &str = "Arto is the Markdown reader on the user's screen. \
Use `open` to put a file in front of the user, `state` to see what is open and where \
the reader is, `goto` to move the reader to a heading, a place or a search hit, and \
`selection` to read what the user has selected. `open` only opens paths under the \
directories this server was started with.";

/// How long `open` waits for an Arto it started to show the document.
#[derive(Debug, Clone, Copy)]
pub struct LaunchWait {
    pub deadline: Duration,
    pub interval: Duration,
}

impl Default for LaunchWait {
    fn default() -> Self {
        Self {
            deadline: Duration::from_secs(30),
            interval: Duration::from_millis(250),
        }
    }
}

/// The MCP server: tools over one running Arto.
#[derive(Clone)]
pub struct Arto<I: Instance> {
    instance: Arc<I>,
    roots: Arc<Roots>,
    server_info: Implementation,
    launch_wait: LaunchWait,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OpenArgs {
    /// Files to read, and at most one directory to show in the file explorer.
    /// Relative paths are read against the directory the server was started in.
    pub paths: Vec<PathBuf>,
    /// Open in a new window rather than where Arto's settings say.
    #[serde(default)]
    pub new_window: bool,
    /// Bring Arto to the front. Left out, Arto opens the file without taking
    /// the keyboard from the application the user is working in.
    #[serde(default)]
    pub activate: bool,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct WindowArgs {
    /// A window number as `state` reports it. Left out, every window for
    /// `state`, and the window the user was last in otherwise.
    #[serde(default)]
    pub window: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GotoArgs {
    /// A window number as `state` reports it; the window the user was last in
    /// when left out.
    #[serde(default)]
    pub window: Option<u32>,
    pub target: Target,
}

/// Where to put the reader.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    /// A heading, by the `id` that `state` lists it with.
    Heading(String),
    /// A place as `state` reports it in `scroll`: the source line of the block
    /// at the top (0 for the top of the document) and how far into that block
    /// the top edge sits, from 0 to just under 1.
    Scroll { line: u32, fraction: f32 },
    /// The top of the document.
    Top,
    /// The end of the document.
    Bottom,
    /// A hit of a search, highlighted in the find panel. `index` counts hits
    /// from zero.
    Search {
        query: String,
        #[serde(default)]
        index: u32,
    },
}

impl From<Target> for arto_lsp::GotoTarget {
    fn from(target: Target) -> Self {
        match target {
            Target::Heading(id) => Self::Heading(id),
            Target::Scroll { line, fraction } => Self::Scroll(ScrollPosition { line, fraction }),
            Target::Top => Self::Top,
            Target::Bottom => Self::Bottom,
            Target::Search { query, index } => Self::Search { query, index },
        }
    }
}

#[tool_router]
impl<I: Instance> Arto<I> {
    pub fn new(instance: I, roots: Roots, server_info: Implementation) -> Self {
        Self {
            instance: Arc::new(instance),
            roots: Arc::new(roots),
            server_info,
            launch_wait: LaunchWait::default(),
            tool_router: Self::tool_router(),
        }
    }

    pub fn with_launch_wait(mut self, launch_wait: LaunchWait) -> Self {
        self.launch_wait = launch_wait;
        self
    }

    /// Show files to the user in Arto, starting Arto if it is not running.
    #[tool(
        description = "Open Markdown files in Arto, on the user's screen, and answer once they are drawn. A directory names the folder the file explorer shows. Only paths under the directories this server was started with are opened."
    )]
    async fn open(
        &self,
        Parameters(args): Parameters<OpenArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = match self.open_request(args) {
            Ok(request) => request,
            Err(message) => return Ok(refusal(message)),
        };
        let call = Call::Apply {
            event: OpenEvent::Open(request.clone()),
            wait_ready: true,
        };
        match self.ask(call).await {
            Err(AskError::NoInstance) => Ok(self.launch(request).await),
            outcome => Ok(answer(outcome)),
        }
    }

    /// What every window shows, and where the reader is in it.
    #[tool(
        description = "List Arto's open windows: each one's number, the file it shows, its theme, its table of contents (headings with the ids `goto` takes), and where the reader is (`scroll`, which `goto` can be handed back). `focused` is the window the user was last in."
    )]
    async fn state(
        &self,
        Parameters(args): Parameters<WindowArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let call = Call::State(StateParams {
            window: args.window.map(WindowKey),
        });
        Ok(answer(self.ask(call).await))
    }

    /// Move the reader within the document a window shows.
    #[tool(
        description = "Scroll an Arto window to a heading, a place `state` reported, the top or the bottom, or a search hit. Answers once the window has been asked to move; call `state` afterwards for where the reader settled."
    )]
    async fn goto(
        &self,
        Parameters(args): Parameters<GotoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let call = Call::Goto(GotoParams {
            window: args.window.map(WindowKey),
            target: args.target.into(),
        });
        Ok(answer(self.ask(call).await))
    }

    /// What the user has selected.
    #[tool(
        description = "Read the text the user has selected in an Arto window. `text` is absent when nothing is selected."
    )]
    async fn selection(
        &self,
        Parameters(args): Parameters<WindowArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let call = Call::Selection(SelectionParams {
            window: args.window.map(WindowKey),
        });
        Ok(answer(self.ask(call).await))
    }
}

impl<I: Instance> Arto<I> {
    /// The request `open` sends, once every path has been admitted.
    fn open_request(&self, args: OpenArgs) -> Result<OpenRequest, String> {
        if args.paths.is_empty() {
            return Err("name at least one file or directory to open".to_string());
        }
        let mut files = Vec::new();
        let mut directories = Vec::new();
        for path in &args.paths {
            let admitted = self
                .roots
                .admit(path)
                .map_err(|refusal| refusal.to_string())?;
            if admitted.is_dir() {
                directories.push(admitted);
            } else {
                files.push(admitted);
            }
        }
        if directories.len() > 1 {
            return Err("name at most one directory: a window shows one folder".to_string());
        }
        Ok(OpenRequest {
            files,
            directory: directories.pop(),
            behavior: args.new_window.then_some(FileOpenBehavior::NewWindow),
            behind: !args.activate,
            window: WindowOptions::default(),
        })
    }

    /// Start Arto on what `open` was asked for, and wait for it to show it.
    async fn launch(&self, request: OpenRequest) -> CallToolResult {
        let paths: Vec<PathBuf> = request
            .files
            .iter()
            .chain(request.directory.iter())
            .cloned()
            .collect();
        if let Err(error) = self.instance.launch(&paths, request.behind) {
            return refusal(format!(
                "Arto is not running, and starting it failed: {error}"
            ));
        }
        let ready = self
            .wait_until_showing(request.files.first().cloned())
            .await;
        success(&serde_json::json!({"ready": ready, "launched": true}))
    }

    /// Whether the Arto just started came to show `file` — or, for a request
    /// that named only a directory, any window — within the deadline.
    async fn wait_until_showing(&self, file: Option<PathBuf>) -> bool {
        let started = tokio::time::Instant::now();
        while started.elapsed() < self.launch_wait.deadline {
            if let Ok(value) = self.ask(Call::State(StateParams::default())).await {
                if let Ok(state) = serde_json::from_value::<StateResult>(value) {
                    let shown = match &file {
                        Some(file) => state
                            .windows
                            .iter()
                            .any(|window| window.document.as_ref() == Some(file)),
                        None => !state.windows.is_empty(),
                    };
                    if shown {
                        return true;
                    }
                }
            }
            tokio::time::sleep(self.launch_wait.interval).await;
        }
        false
    }

    /// The socket is blocking, and a call that waits for a window to draw
    /// can take seconds; on a thread of its own it holds up nothing else.
    async fn ask(&self, call: Call) -> Result<serde_json::Value, AskError> {
        let instance = Arc::clone(&self.instance);
        tokio::task::spawn_blocking(move || instance.ask(&call))
            .await
            .unwrap_or(Err(AskError::Unanswered))
    }
}

#[tool_handler(router = self.tool_router)]
impl<I: Instance> ServerHandler for Arto<I> {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(self.server_info.clone())
            .with_instructions(INSTRUCTIONS)
    }
}

/// What Arto answered, or why it did not, as the agent will read it.
///
/// Every failure is a tool error rather than a protocol one: a client shows
/// the agent a tool error's text, and a protocol error only as "the tool
/// failed".
fn answer(outcome: Result<serde_json::Value, AskError>) -> CallToolResult {
    match outcome {
        Ok(value) => success(&value),
        Err(AskError::Refused(error)) => refusal(error.message),
        Err(error) => refusal(error.to_string()),
    }
}

fn success(value: &serde_json::Value) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(value.to_string())])
}

fn refusal(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

#[cfg(test)]
mod tests {
    use super::*;
    use arto_lsp::{ErrorCode, GotoTarget, ResponseError, WindowState};
    use arto_types::Theme;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// A stand-in Arto: it records what it was asked and answers from a
    /// script, `NoInstance` once the script runs out.
    #[derive(Clone, Default)]
    struct Fake(Arc<Script>);

    #[derive(Default)]
    struct Script {
        asked: Mutex<Vec<Call>>,
        answers: Mutex<VecDeque<Result<serde_json::Value, AskError>>>,
        launched: Mutex<Vec<(Vec<PathBuf>, bool)>>,
    }

    impl Fake {
        fn answering(
            answers: impl IntoIterator<Item = Result<serde_json::Value, AskError>>,
        ) -> Self {
            let fake = Self::default();
            fake.0.answers.lock().unwrap().extend(answers);
            fake
        }

        fn asked(&self) -> Vec<Call> {
            self.0.asked.lock().unwrap().clone()
        }

        fn launched(&self) -> Vec<(Vec<PathBuf>, bool)> {
            self.0.launched.lock().unwrap().clone()
        }
    }

    impl Instance for Fake {
        fn ask(&self, call: &Call) -> Result<serde_json::Value, AskError> {
            self.0.asked.lock().unwrap().push(call.clone());
            self.0
                .answers
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Err(AskError::NoInstance))
        }

        fn launch(&self, paths: &[PathBuf], behind: bool) -> std::io::Result<()> {
            self.0
                .launched
                .lock()
                .unwrap()
                .push((paths.to_vec(), behind));
            Ok(())
        }
    }

    struct Project {
        _temp: tempfile::TempDir,
        root: PathBuf,
    }

    /// `README.md` and `docs/` inside the root, `secret.md` beside it.
    fn project() -> Project {
        let temp = tempfile::tempdir().unwrap();
        let top = temp.path().canonicalize().unwrap();
        let root = top.join("project");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::write(root.join("README.md"), "# Readme").unwrap();
        std::fs::write(top.join("secret.md"), "# Secret").unwrap();
        Project { _temp: temp, root }
    }

    fn server(fake: &Fake, project: &Project) -> Arto<Fake> {
        Arto::new(
            fake.clone(),
            Roots::new([project.root.clone()], &project.root),
            Implementation::new("arto", "test"),
        )
        .with_launch_wait(LaunchWait {
            deadline: Duration::from_secs(2),
            interval: Duration::from_millis(1),
        })
    }

    fn open_args(paths: &[&str]) -> Parameters<OpenArgs> {
        Parameters(OpenArgs {
            paths: paths.iter().map(PathBuf::from).collect(),
            new_window: false,
            activate: false,
        })
    }

    fn text(result: &CallToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|block| block.as_text().map(|text| text.text.clone()))
            .collect()
    }

    fn is_error(result: &CallToolResult) -> bool {
        result.is_error == Some(true)
    }

    fn showing(file: &std::path::Path) -> serde_json::Value {
        serde_json::to_value(StateResult {
            windows: vec![WindowState {
                window: WindowKey(1),
                document: Some(file.to_path_buf()),
                theme: Theme::Light,
                scroll: ScrollPosition::default(),
                headings: vec![],
            }],
            focused: Some(WindowKey(1)),
        })
        .unwrap()
    }

    #[test]
    fn the_tools_are_named_for_what_they_do() {
        let mut names: Vec<String> = Arto::<Fake>::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["goto", "open", "selection", "state"]);
    }

    #[tokio::test]
    async fn open_hands_arto_the_canonical_path_and_waits_for_the_drawing() {
        let project = project();
        let fake = Fake::answering([Ok(json!({"ready": true}))]);

        let result = server(&fake, &project)
            .open(open_args(&["./README.md"]))
            .await
            .unwrap();

        assert!(!is_error(&result), "{}", text(&result));
        assert_eq!(text(&result), r#"{"ready":true}"#);
        assert_eq!(
            fake.asked(),
            [Call::Apply {
                event: OpenEvent::Open(OpenRequest {
                    files: vec![project.root.join("README.md")],
                    directory: None,
                    behavior: None,
                    behind: true,
                    window: WindowOptions::default(),
                }),
                wait_ready: true,
            }]
        );
    }

    #[tokio::test]
    async fn open_takes_the_keyboard_only_when_asked_to() {
        let project = project();
        let fake = Fake::answering([Ok(json!({"ready": true}))]);
        let args = Parameters(OpenArgs {
            paths: vec![PathBuf::from("README.md")],
            new_window: true,
            activate: true,
        });

        server(&fake, &project).open(args).await.unwrap();

        let Call::Apply {
            event: OpenEvent::Open(request),
            ..
        } = &fake.asked()[0]
        else {
            panic!("not an open: {:?}", fake.asked());
        };
        assert!(!request.behind);
        assert_eq!(request.behavior, Some(FileOpenBehavior::NewWindow));
    }

    #[tokio::test]
    async fn a_directory_is_opened_as_the_folder_the_explorer_shows() {
        let project = project();
        let fake = Fake::answering([Ok(json!({"ready": true}))]);

        server(&fake, &project)
            .open(open_args(&["docs", "README.md"]))
            .await
            .unwrap();

        let Call::Apply {
            event: OpenEvent::Open(request),
            ..
        } = &fake.asked()[0]
        else {
            panic!("not an open: {:?}", fake.asked());
        };
        assert_eq!(request.files, [project.root.join("README.md")]);
        assert_eq!(request.directory, Some(project.root.join("docs")));
    }

    #[tokio::test]
    async fn a_path_outside_the_roots_is_refused_and_arto_is_never_asked() {
        let project = project();
        let fake = Fake::default();

        let result = server(&fake, &project)
            .open(open_args(&["../secret.md"]))
            .await
            .unwrap();

        assert!(is_error(&result));
        assert!(
            text(&result).contains(&project.root.display().to_string()),
            "the refusal names where the server may open: {}",
            text(&result)
        );
        assert!(fake.asked().is_empty());
        assert!(fake.launched().is_empty());
    }

    #[tokio::test]
    async fn opening_nothing_is_refused() {
        let project = project();
        let fake = Fake::default();
        let result = server(&fake, &project).open(open_args(&[])).await.unwrap();
        assert!(is_error(&result));
        assert!(fake.asked().is_empty());
    }

    #[tokio::test]
    async fn two_directories_are_refused() {
        let project = project();
        let fake = Fake::default();
        let result = server(&fake, &project)
            .open(open_args(&["docs", "notes"]))
            .await
            .unwrap();
        assert!(is_error(&result));
        assert!(fake.asked().is_empty());
    }

    #[tokio::test]
    async fn open_starts_arto_when_it_is_not_running_and_waits_for_the_document() {
        let project = project();
        let readme = project.root.join("README.md");
        let fake = Fake::answering([
            Err(AskError::NoInstance),
            // Still starting, then up with nothing open yet, then showing it.
            Err(AskError::NoInstance),
            Ok(serde_json::to_value(StateResult::default()).unwrap()),
            Ok(showing(&readme)),
        ]);

        let result = server(&fake, &project)
            .open(open_args(&["README.md"]))
            .await
            .unwrap();

        assert!(!is_error(&result), "{}", text(&result));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text(&result)).unwrap(),
            json!({"ready": true, "launched": true})
        );
        assert_eq!(fake.launched(), [(vec![readme], true)]);
    }

    #[tokio::test]
    async fn an_arto_that_was_started_and_never_showed_the_document_is_not_ready() {
        let project = project();
        let fake = Fake::answering([Err(AskError::NoInstance)]);
        let server = server(&fake, &project).with_launch_wait(LaunchWait {
            deadline: Duration::from_millis(20),
            interval: Duration::from_millis(1),
        });

        let result = server.open(open_args(&["README.md"])).await.unwrap();

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text(&result)).unwrap(),
            json!({"ready": false, "launched": true})
        );
    }

    #[tokio::test]
    async fn state_is_arto_s_own_answer() {
        let project = project();
        let readme = project.root.join("README.md");
        let fake = Fake::answering([Ok(showing(&readme))]);

        let result = server(&fake, &project)
            .state(Parameters(WindowArgs { window: Some(1) }))
            .await
            .unwrap();

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text(&result)).unwrap(),
            showing(&readme)
        );
        assert_eq!(
            fake.asked(),
            [Call::State(StateParams {
                window: Some(WindowKey(1))
            })]
        );
    }

    #[tokio::test]
    async fn reading_the_state_never_starts_arto() {
        let project = project();
        let fake = Fake::default();
        let result = server(&fake, &project)
            .state(Parameters(WindowArgs::default()))
            .await
            .unwrap();
        assert!(is_error(&result));
        assert!(text(&result).contains("not running"), "{}", text(&result));
        assert!(fake.launched().is_empty());
    }

    #[tokio::test]
    async fn goto_takes_the_protocol_s_own_targets() {
        let project = project();
        let fake = Fake::answering([Ok(json!({"window": 2}))]);
        let args: GotoArgs = serde_json::from_value(json!({
            "window": 2,
            "target": {"search": {"query": "install", "index": 1}}
        }))
        .unwrap();

        server(&fake, &project)
            .goto(Parameters(args))
            .await
            .unwrap();

        assert_eq!(
            fake.asked(),
            [Call::Goto(GotoParams {
                window: Some(WindowKey(2)),
                target: GotoTarget::Search {
                    query: "install".into(),
                    index: 1,
                },
            })]
        );
    }

    #[test]
    fn a_scroll_that_state_reported_is_a_target_goto_takes_back() {
        // The round trip the two tools exist for: `scroll` from `state`,
        // wrapped as the target, without reshaping it.
        let reported = serde_json::to_value(ScrollPosition {
            line: 42,
            fraction: 0.25,
        })
        .unwrap();
        let target: Target = serde_json::from_value(json!({ "scroll": reported })).unwrap();
        assert_eq!(
            GotoTarget::from(target),
            GotoTarget::Scroll(ScrollPosition {
                line: 42,
                fraction: 0.25,
            })
        );
    }

    #[tokio::test]
    async fn a_refusal_from_arto_reaches_the_agent_as_its_message() {
        let project = project();
        let fake = Fake::answering([Err(AskError::Refused(ResponseError::new(
            ErrorCode::InvalidParams,
            "no window 9 is open",
        )))]);

        let result = server(&fake, &project)
            .selection(Parameters(WindowArgs { window: Some(9) }))
            .await
            .unwrap();

        assert!(is_error(&result));
        assert_eq!(text(&result), "no window 9 is open");
    }
}
