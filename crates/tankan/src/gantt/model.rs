//! gantt の AST（日付解決済み）

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TaskTags {
    pub done: bool,
    pub active: bool,
    pub crit: bool,
    pub milestone: bool,
}

#[derive(Debug)]
pub(crate) struct Task {
    pub name: String,
    /// 開始日（通算日）
    pub start: i64,
    /// 終了日（**排他**。幅 = end - start 日）
    pub end: i64,
    pub tags: TaskTags,
}

#[derive(Debug)]
pub(crate) struct Section {
    /// section 行より前のタスクは名前空セクションに入る
    pub name: String,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TickInterval {
    Day(u32),
    Week(u32),
    Month(u32),
}

#[derive(Debug, Default)]
pub(crate) struct GanttDiagram {
    pub title: Option<String>,
    pub axis_format: Option<String>,
    pub tick: Option<TickInterval>,
    pub sections: Vec<Section>,
    /// 除外日（通算日）の昇順リスト
    pub excluded_days: Vec<i64>,
}
