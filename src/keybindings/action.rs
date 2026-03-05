use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // Mode transitions
    EnterNormalMode,
    EnterInsertMode,
    EnterVisualMode,
    EnterVisualLineMode,
    EnterCommandMode,
    EnterSearchMode,
    Append,
    OpenLineBelow,
    OpenLineAbove,

    // Navigation
    MoveCursorLeft,
    MoveCursorDown,
    MoveCursorUp,
    MoveCursorRight,
    MoveWordForward,
    #[serde(rename = "move_word_forward_big")]
    MoveWORDForward,
    MoveWordBackward,
    #[serde(rename = "move_word_backward_big")]
    MoveWORDBackward,
    MoveToLineStart,
    MoveToFirstNonWhitespace,
    MoveToLineEnd,
    GotoFirstLine,
    GotoLastLine,
    GotoMatchingBracket,
    FindCharForward,
    FindCharBackward,
    GoToDefinition,
    JumpBack,

    // Editing
    DeleteChar,
    DeleteLine,
    DeleteWord,
    DeleteToLineEnd,
    DeleteToLineStart,
    YankLine,
    YankWord,
    YankToLineEnd,
    YankToLineStart,
    PasteAfter,
    PasteBefore,
    Undo,
    Redo,
    InsertNewline,
    Backspace,

    // Visual mode
    DeleteVisualSelection,
    ChangeVisualSelection,
    AppendAfterVisualSelection,
    YankVisualSelection,

    // Search
    SearchNext,
    SearchPrev,

    // Scroll
    PageDown,
    PageUp,
    HalfPageDown,
    HalfPageUp,

    // Multi-cursor
    SelectWordOrNextMatch,

    // Splits
    SplitHorizontal,
    SplitVertical,
    FocusNextView,
    FocusDirectionLeft,
    FocusDirectionDown,
    FocusDirectionUp,
    FocusDirectionRight,
    CloseCurrentView,

    // Fuzzy finder / pickers
    FuzzyFindFiles,
    FuzzyGrep,
    ThemePicker,
    BufferPicker,

    // Completion
    AcceptCompletion,
    CompletionNext,
    CompletionPrev,
    DismissCompletion,
    AcceptGhostText,

    // File operations
    SaveFile,

    // Lifecycle
    Exit,

    // Git
    OpenGit,

    // Help
    ShowKeybindings,

    // Settings
    OpenSettings,

    // Explicitly unbound
    Noop,
}
