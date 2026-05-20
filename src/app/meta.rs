use super::*;

impl App {
    pub fn toggle_flag_pill(&mut self, idx: usize) {
        self.flag_active[idx] = !self.flag_active[idx];
        if self.flag_active[idx] {
            self.active_highlight = Some(idx);
        } else {
            self.active_highlight = self.flag_active.iter().position(|&x| x);
        }
    }

    /// Toggle the active flag on the currently selected task. Returns true if a change was made.
    pub fn flag_selected_task(&mut self) -> bool {
        let Some(flag_idx) = self.active_highlight else { return false; };
        let Some(id) = self.selected_task_id(self.focus) else { return false; };
        if let Some(task) = self.task_mut(id) {
            task.flags ^= 1u8 << flag_idx;
            true
        } else {
            false
        }
    }

    pub fn begin_flag_clear(&mut self) {
        let active: Vec<usize> = (0..3).filter(|&i| self.flag_active[i]).collect();
        if active.is_empty() { return; }
        let nums: String = active.iter().map(|&i| (i + 1).to_string()).collect::<Vec<_>>().join(", ");
        self.flag_clear_confirm = true;
        self.status_message = Some(format!(
            "Clear flag {} highlights? (Enter to confirm, Esc to cancel)", nums
        ));
    }

    pub fn confirm_flag_clear(&mut self) {
        for i in 0..3 {
            if self.flag_active[i] {
                let mask = !(1u8 << i);
                for task in &mut self.tasks {
                    task.flags &= mask;
                }
            }
        }
        self.flag_clear_confirm = false;
        self.status_message = None;
    }

    pub fn cancel_flag_clear(&mut self) {
        self.flag_clear_confirm = false;
        self.status_message = None;
    }


}
