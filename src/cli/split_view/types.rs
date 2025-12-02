/// Mode for the detail view
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Log,
    Diff,
}

/// Which panel is focused
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusedPanel {
    Models,
    Details,
}

/// Input mode for the application
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    Confirm,
    ConfirmCancel,
}

/// Result from the split view selection
#[derive(Debug, Clone)]
pub enum SplitViewResult {
    Apply(usize),
    Cancel,
}
