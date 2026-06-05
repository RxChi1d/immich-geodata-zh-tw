use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug, Clone)]
pub struct DependencyLogger {
    stage: String,
}

impl DependencyLogger {
    pub fn new(stage: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
        }
    }

    pub fn info(&self, message: &str) -> String {
        self.line("INFO", message)
    }

    pub fn warning(&self, message: &str) -> String {
        self.line("WARNING", message)
    }

    pub fn error(&self, message: &str) -> String {
        self.line("ERROR", message)
    }

    fn line(&self, level: &str, message: &str) -> String {
        format!("level={level} stage={} {message}", self.stage)
    }
}

#[derive(Debug, Clone)]
pub struct ProgressReporter {
    stage: String,
    total: u64,
    bar: ProgressBar,
}

impl ProgressReporter {
    pub fn new(stage: impl Into<String>, total: u64) -> Self {
        let stage = stage.into();
        let bar = ProgressBar::hidden();
        bar.set_length(total);
        if let Ok(style) = ProgressStyle::with_template(
            "{prefix:.bold} [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len}",
        ) {
            bar.set_style(style);
        }
        bar.set_prefix(stage.clone());
        Self { stage, total, bar }
    }

    pub fn started_line(&self) -> String {
        format!("progress_start stage={} total={}", self.stage, self.total)
    }

    pub fn step_line(&self, current: u64) -> String {
        format!(
            "progress stage={} current={} total={}",
            self.stage, current, self.total
        )
    }

    pub fn finished_line(&self) -> String {
        format!("progress_finish stage={} total={}", self.stage, self.total)
    }

    pub fn start(&self) {
        println!("{}", self.started_line());
    }

    pub fn step(&self, current: u64) {
        self.bar.set_position(current);
        println!("{}", self.step_line(current));
    }

    pub fn finish(&self) {
        self.bar.finish_and_clear();
        println!("{}", self.finished_line());
    }
}
