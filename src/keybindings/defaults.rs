use super::action::Action;
use super::key::{Key, KeyCombo};
use super::registry::{KeybindingRegistry, ModeKey};

pub fn build_default_registry() -> KeybindingRegistry {
    let mut reg = KeybindingRegistry::new();
    register_normal_defaults(&mut reg);
    register_insert_defaults(&mut reg);
    register_visual_defaults(&mut reg);
    register_visual_line_defaults(&mut reg);
    register_completion_defaults(&mut reg);
    register_normie_defaults(&mut reg);
    reg
}

fn register_normal_defaults(reg: &mut KeybindingRegistry) {
    let m = ModeKey::Normal;

    // Single-key bindings
    reg.bind(m.clone(), vec![KeyCombo::char('q')], Action::Exit);
    reg.bind(m.clone(), vec![KeyCombo::char('h')], Action::MoveCursorLeft);
    reg.bind(m.clone(), vec![KeyCombo::char('j')], Action::MoveCursorDown);
    reg.bind(m.clone(), vec![KeyCombo::char('k')], Action::MoveCursorUp);
    reg.bind(m.clone(), vec![KeyCombo::char('l')], Action::MoveCursorRight);
    reg.bind(m.clone(), vec![KeyCombo::char('w')], Action::MoveWordForward);
    reg.bind(m.clone(), vec![KeyCombo::char('W')], Action::MoveWORDForward);
    reg.bind(m.clone(), vec![KeyCombo::char('b')], Action::MoveWordBackward);
    reg.bind(m.clone(), vec![KeyCombo::char('B')], Action::MoveWORDBackward);
    reg.bind(m.clone(), vec![KeyCombo::char('0')], Action::MoveToLineStart);
    reg.bind(m.clone(), vec![KeyCombo::char('^')], Action::MoveToFirstNonWhitespace);
    reg.bind(m.clone(), vec![KeyCombo::char('_')], Action::MoveToFirstNonWhitespace);
    reg.bind(m.clone(), vec![KeyCombo::char('$')], Action::MoveToLineEnd);
    reg.bind(m.clone(), vec![KeyCombo::char('G')], Action::GotoLastLine);
    reg.bind(m.clone(), vec![KeyCombo::char('x')], Action::DeleteChar);
    reg.bind(m.clone(), vec![KeyCombo::char('p')], Action::PasteAfter);
    reg.bind(m.clone(), vec![KeyCombo::char('P')], Action::PasteBefore);
    reg.bind(m.clone(), vec![KeyCombo::char('%')], Action::GotoMatchingBracket);
    reg.bind(m.clone(), vec![KeyCombo::char('u')], Action::Undo);
    reg.bind(m.clone(), vec![KeyCombo::char('v')], Action::EnterVisualMode);
    reg.bind(m.clone(), vec![KeyCombo::char('V')], Action::EnterVisualLineMode);
    reg.bind(m.clone(), vec![KeyCombo::char(':')], Action::EnterCommandMode);
    reg.bind(m.clone(), vec![KeyCombo::char('/')], Action::EnterSearchMode);
    reg.bind(m.clone(), vec![KeyCombo::char('n')], Action::SearchNext);
    reg.bind(m.clone(), vec![KeyCombo::char('N')], Action::SearchPrev);
    reg.bind(m.clone(), vec![KeyCombo::char('i')], Action::EnterInsertMode);
    reg.bind(m.clone(), vec![KeyCombo::char('a')], Action::Append);
    reg.bind(m.clone(), vec![KeyCombo::char('o')], Action::OpenLineBelow);
    reg.bind(m.clone(), vec![KeyCombo::char('O')], Action::OpenLineAbove);
    reg.bind(m.clone(), vec![KeyCombo::char('f')], Action::FindCharForward);
    reg.bind(m.clone(), vec![KeyCombo::char('F')], Action::FindCharBackward);

    // Arrow keys
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Left)], Action::MoveCursorLeft);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Down)], Action::MoveCursorDown);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Up)], Action::MoveCursorUp);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Right)], Action::MoveCursorRight);

    // Ctrl bindings
    reg.bind(m.clone(), vec![KeyCombo::ctrl('r')], Action::Redo);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('f')], Action::PageDown);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('b')], Action::PageUp);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('d')], Action::HalfPageDown);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('u')], Action::HalfPageUp);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('n')], Action::SelectWordOrNextMatch);

    // Jump list
    reg.bind(m.clone(), vec![KeyCombo::ctrl('o')], Action::JumpBack);

    // Multi-key sequences
    reg.bind(m.clone(), vec![KeyCombo::char('g'), KeyCombo::char('g')], Action::GotoFirstLine);
    reg.bind(m.clone(), vec![KeyCombo::char('g'), KeyCombo::char('d')], Action::GoToDefinition);
    reg.bind(m.clone(), vec![KeyCombo::char('d'), KeyCombo::char('d')], Action::DeleteLine);
    reg.bind(m.clone(), vec![KeyCombo::char('d'), KeyCombo::char('w')], Action::DeleteWord);
    reg.bind(m.clone(), vec![KeyCombo::char('d'), KeyCombo::char('$')], Action::DeleteToLineEnd);
    reg.bind(m.clone(), vec![KeyCombo::char('d'), KeyCombo::char('0')], Action::DeleteToLineStart);
    reg.bind(m.clone(), vec![KeyCombo::char('y'), KeyCombo::char('y')], Action::YankLine);
    reg.bind(m.clone(), vec![KeyCombo::char('y'), KeyCombo::char('w')], Action::YankWord);
    reg.bind(m.clone(), vec![KeyCombo::char('y'), KeyCombo::char('$')], Action::YankToLineEnd);
    reg.bind(m.clone(), vec![KeyCombo::char('y'), KeyCombo::char('0')], Action::YankToLineStart);

    // Space sequences
    reg.bind(m.clone(), vec![KeyCombo::char(' '), KeyCombo::char('f'), KeyCombo::char('f')], Action::FuzzyFindFiles);
    reg.bind(m.clone(), vec![KeyCombo::char(' '), KeyCombo::char('f'), KeyCombo::char('t')], Action::ThemePicker);
    reg.bind(m.clone(), vec![KeyCombo::char(' '), KeyCombo::char('b'), KeyCombo::char('b')], Action::BufferPicker);
    reg.bind(m.clone(), vec![KeyCombo::char(' '), KeyCombo::char('/')], Action::FuzzyGrep);

    // Help
    reg.bind(m.clone(), vec![KeyCombo::special(Key::F(1))], Action::ShowKeybindings);

    // Git
    reg.bind(m.clone(), vec![KeyCombo::char(' '), KeyCombo::char('g')], Action::OpenGit);

    // File tree
    reg.bind(m.clone(), vec![KeyCombo::char(' '), KeyCombo::char('e')], Action::OpenFileTree);

    // Settings
    reg.bind(m.clone(), vec![KeyCombo::special(Key::F(2))], Action::OpenSettings);

    // Ctrl+w sequences
    reg.bind(m.clone(), vec![KeyCombo::ctrl('w'), KeyCombo::char('s')], Action::SplitHorizontal);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('w'), KeyCombo::char('v')], Action::SplitVertical);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('w'), KeyCombo::char('w')], Action::FocusNextView);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('w'), KeyCombo::char('h')], Action::FocusDirectionLeft);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('w'), KeyCombo::char('j')], Action::FocusDirectionDown);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('w'), KeyCombo::char('k')], Action::FocusDirectionUp);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('w'), KeyCombo::char('l')], Action::FocusDirectionRight);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('w'), KeyCombo::char('q')], Action::CloseCurrentView);
}

fn register_insert_defaults(reg: &mut KeybindingRegistry) {
    let m = ModeKey::Insert;
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Esc)], Action::EnterNormalMode);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Enter)], Action::InsertNewline);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Backspace)], Action::Backspace);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('l')], Action::AcceptGhostText);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::F(1))], Action::ShowKeybindings);
}

fn register_visual_defaults(reg: &mut KeybindingRegistry) {
    let m = ModeKey::Visual;
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Esc)], Action::EnterNormalMode);
    reg.bind(m.clone(), vec![KeyCombo::char('h')], Action::MoveCursorLeft);
    reg.bind(m.clone(), vec![KeyCombo::char('j')], Action::MoveCursorDown);
    reg.bind(m.clone(), vec![KeyCombo::char('k')], Action::MoveCursorUp);
    reg.bind(m.clone(), vec![KeyCombo::char('l')], Action::MoveCursorRight);
    reg.bind(m.clone(), vec![KeyCombo::char('w')], Action::MoveWordForward);
    reg.bind(m.clone(), vec![KeyCombo::char('W')], Action::MoveWORDForward);
    reg.bind(m.clone(), vec![KeyCombo::char('b')], Action::MoveWordBackward);
    reg.bind(m.clone(), vec![KeyCombo::char('B')], Action::MoveWORDBackward);
    reg.bind(m.clone(), vec![KeyCombo::char('0')], Action::MoveToLineStart);
    reg.bind(m.clone(), vec![KeyCombo::char('$')], Action::MoveToLineEnd);
    reg.bind(m.clone(), vec![KeyCombo::char('%')], Action::GotoMatchingBracket);
    reg.bind(m.clone(), vec![KeyCombo::char('G')], Action::GotoLastLine);
    reg.bind(m.clone(), vec![KeyCombo::char('d')], Action::DeleteVisualSelection);
    reg.bind(m.clone(), vec![KeyCombo::char('x')], Action::DeleteVisualSelection);
    reg.bind(m.clone(), vec![KeyCombo::char('c')], Action::ChangeVisualSelection);
    reg.bind(m.clone(), vec![KeyCombo::char('s')], Action::ChangeVisualSelection);
    reg.bind(m.clone(), vec![KeyCombo::char('a')], Action::AppendAfterVisualSelection);
    reg.bind(m.clone(), vec![KeyCombo::char('y')], Action::YankVisualSelection);
    reg.bind(m.clone(), vec![KeyCombo::char('n')], Action::SearchNext);
    reg.bind(m.clone(), vec![KeyCombo::char('N')], Action::SearchPrev);
    reg.bind(m.clone(), vec![KeyCombo::char('V')], Action::EnterVisualLineMode);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('n')], Action::SelectWordOrNextMatch);

    // Arrow keys
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Left)], Action::MoveCursorLeft);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Down)], Action::MoveCursorDown);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Up)], Action::MoveCursorUp);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Right)], Action::MoveCursorRight);
}

fn register_visual_line_defaults(reg: &mut KeybindingRegistry) {
    let m = ModeKey::VisualLine;
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Esc)], Action::EnterNormalMode);
    reg.bind(m.clone(), vec![KeyCombo::char('j')], Action::MoveCursorDown);
    reg.bind(m.clone(), vec![KeyCombo::char('k')], Action::MoveCursorUp);
    reg.bind(m.clone(), vec![KeyCombo::char('G')], Action::GotoLastLine);
    reg.bind(m.clone(), vec![KeyCombo::char('d')], Action::DeleteVisualSelection);
    reg.bind(m.clone(), vec![KeyCombo::char('x')], Action::DeleteVisualSelection);
    reg.bind(m.clone(), vec![KeyCombo::char('y')], Action::YankVisualSelection);
    reg.bind(m.clone(), vec![KeyCombo::char('v')], Action::EnterVisualMode);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Down)], Action::MoveCursorDown);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Up)], Action::MoveCursorUp);
}

fn register_normie_defaults(reg: &mut KeybindingRegistry) {
    let m = ModeKey::Normie;

    // Text entry keys (Enter, Backspace) are bound; Char input is handled as fallback
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Enter)], Action::InsertNewline);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Backspace)], Action::Backspace);

    // Arrow key navigation
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Left)], Action::MoveCursorLeft);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Right)], Action::MoveCursorRight);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Up)], Action::MoveCursorUp);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Down)], Action::MoveCursorDown);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Home)], Action::MoveToLineStart);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::End)], Action::MoveToLineEnd);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::PageUp)], Action::PageUp);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::PageDown)], Action::PageDown);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Delete)], Action::DeleteChar);

    // Ctrl shortcuts
    reg.bind(m.clone(), vec![KeyCombo::ctrl('s')], Action::SaveFile);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('q')], Action::Exit);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('z')], Action::Undo);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('y')], Action::Redo);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('f')], Action::EnterSearchMode);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('d')], Action::SelectWordOrNextMatch);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('l')], Action::AcceptGhostText);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('o')], Action::FuzzyFindFiles);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('p')], Action::FuzzyGrep);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('w')], Action::CloseCurrentView);

    // Help, Settings & Git
    reg.bind(m.clone(), vec![KeyCombo::special(Key::F(1))], Action::ShowKeybindings);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::F(2))], Action::OpenSettings);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('g')], Action::OpenGit);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('e')], Action::OpenFileTree);

    // Word navigation with Ctrl+arrows
    reg.bind(m.clone(), vec![KeyCombo { key: Key::Left, ctrl: true, alt: false }], Action::MoveWordBackward);
    reg.bind(m.clone(), vec![KeyCombo { key: Key::Right, ctrl: true, alt: false }], Action::MoveWordForward);
    reg.bind(m.clone(), vec![KeyCombo { key: Key::Home, ctrl: true, alt: false }], Action::GotoFirstLine);
    reg.bind(m.clone(), vec![KeyCombo { key: Key::End, ctrl: true, alt: false }], Action::GotoLastLine);
}

fn register_completion_defaults(reg: &mut KeybindingRegistry) {
    let m = ModeKey::Completion;
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Enter)], Action::AcceptCompletion);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('n')], Action::CompletionNext);
    reg.bind(m.clone(), vec![KeyCombo::ctrl('p')], Action::CompletionPrev);
    reg.bind(m.clone(), vec![KeyCombo::special(Key::Esc)], Action::DismissCompletion);
}
