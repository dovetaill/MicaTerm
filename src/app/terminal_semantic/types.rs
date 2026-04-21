//! Shared semantic span types for terminal annotations.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum SemanticPriority {
    Low,
    #[default]
    Normal,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticStyleRole {
    InputPrompt,
    InputCommand,
    InputSubcommand,
    InputOption,
    InputArgument,
    InputString,
    InputPath,
    InputVariable,
    InputInvalidCommand,
    InputOperator,
    OutputJson,
    OutputXml,
    OutputUrl,
    OutputFilePath,
    OutputLineColumn,
    OutputIpPort,
    OutputTimestamp,
    OutputLevelError,
    OutputLevelWarn,
    OutputLevelInfo,
    OutputLevelDebug,
    OutputSuccessKeyword,
    OutputFailureKeyword,
    OutputGrepMatch,
    OutputGitAdded,
    OutputGitRemoved,
    OutputGitHunk,
    OutputJsonKey,
    OutputJsonString,
    OutputJsonNumber,
    OutputJsonBoolean,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticSpan {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub role: SemanticStyleRole,
    pub priority: SemanticPriority,
}

impl SemanticSpan {
    pub fn new(
        row: u32,
        start_col: u32,
        end_col: u32,
        role: SemanticStyleRole,
        priority: SemanticPriority,
    ) -> Self {
        Self {
            row,
            start_col,
            end_col,
            role,
            priority,
        }
    }
}
