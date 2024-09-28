use anyhow::Result;
use chrono::Local;

use crate::backend::database::update_task_in_db;
use crate::backend::task::Status;
use crate::display::add::{EntryMode, Inputs, Stage};
use crate::display::tui::App;

impl App {
    pub fn quick_add(&mut self) {
        // Basically set us up to only enter into Name input
        self.add_stage = Stage::Name;
        self.entry_mode = EntryMode::QuickAdd;
        self.add_popup = !self.add_popup;
        self.character_index = 0;
        self.inputs = Inputs::default();
    }

    pub fn quick_status(&mut self) -> Result<()> {
        // Mark as complete, or if already complete then open
        let current_selection = match self.tasklist.state.selected() {
            Some(n) => n,
            None => return Ok(()),
        };

        //let current_task = &self.tasklist.tasks[current_selection];

        if self.tasklist.tasks[current_selection].status == Status::Completed {
            self.tasklist.tasks[current_selection].status = Status::Open
        } else {
            self.tasklist.tasks[current_selection].status = Status::Completed;
            self.tasklist.tasks[current_selection].completed_on = Some(Local::now());
        }

        update_task_in_db(&self.conn, &self.tasklist.tasks[current_selection])?;
        self.update_tasklist()?;

        self.tasklist.state.select(Some(current_selection));
        Ok(())
    }
}
