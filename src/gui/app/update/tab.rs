use super::super::shortcuts::ShortcutAction;
use super::super::{App, Message, SETTINGS_TAB_INDEX};
use crate::gui::settings::SettingsDraft;
use crate::gui::tab::ShellKind;
use crate::terminal::TerminalTheme;
use iced::Task;
use iced::keyboard::{Key, Modifiers};

impl App {
    pub(in crate::gui) fn create_tab(&mut self, shell: ShellKind) -> Task<Message> {
        let Some(sender) = self.pty_sender.clone() else {
            eprintln!("PTY output channel not ready");
            return Task::none();
        };

        let (cols, rows) = self.grid_for_size(self.window_size);
        let theme = TerminalTheme::from_config(&self.config);
        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let new_tab =
            crate::gui::tab::TerminalTab::from_shell(shell, cols, rows, theme, tab_id, sender);
        self.tabs.push(new_tab);
        self.active_tab = self.tabs.len() - 1;
        self.show_shell_picker = false;
        self.shell_picker_selected = 0;
        Task::none()
    }

    pub(super) fn handle_close_tab(&mut self, index: usize) {
        if index == SETTINGS_TAB_INDEX {
            self.settings_open = false;
            if self.active_tab == SETTINGS_TAB_INDEX {
                self.active_tab = self.tabs.len().saturating_sub(1);
            }
        } else if index < self.tabs.len() {
            self.tabs.remove(index);

            if self.active_tab != SETTINGS_TAB_INDEX {
                if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                    self.active_tab = self.tabs.len() - 1;
                }
                if self.tabs.is_empty() {
                    self.active_tab = 0;
                }
            }
        }
    }

    pub(in crate::gui) fn shell_picker_option_count(&self) -> usize {
        let ssh_count = self.config.ssh_profiles.len();

        #[cfg(target_family = "unix")]
        {
            1 + ssh_count + 1
        }

        #[cfg(target_family = "windows")]
        {
            2 + ssh_count + 1
        }
    }

    pub(super) fn shift_shell_picker_selection(&mut self, delta: isize) {
        let count = self.shell_picker_option_count() as isize;
        if count <= 0 {
            return;
        }

        let next = (self.shell_picker_selected as isize + delta).rem_euclid(count) as usize;
        self.shell_picker_selected = next;
    }

    pub(super) fn confirm_shell_picker_selection(&mut self) -> Task<Message> {
        let selected = self.shell_picker_selected;

        #[cfg(target_family = "unix")]
        let shell_count = 1usize;
        #[cfg(target_family = "windows")]
        let shell_count = 2usize;

        if selected < shell_count {
            #[cfg(target_family = "unix")]
            {
                return self.create_tab(ShellKind::Zsh);
            }

            #[cfg(target_family = "windows")]
            {
                return match selected {
                    0 => self.create_tab(ShellKind::Cmd),
                    _ => self.create_tab(ShellKind::PowerShell),
                };
            }
        }

        let ssh_index = selected - shell_count;
        if ssh_index < self.config.ssh_profiles.len() {
            let profile = self.config.ssh_profiles[ssh_index].clone();
            return self.create_tab(ShellKind::Ssh(profile));
        }

        self.show_shell_picker = false;
        self.shell_picker_selected = 0;
        Task::none()
    }

    pub(super) fn handle_app_shortcut(
        &mut self,
        key: &Key,
        modifiers: Modifiers,
    ) -> Option<Task<Message>> {
        let action = ShortcutAction::resolve(key, modifiers, &self.config.shortcuts)?;

        match action {
            ShortcutAction::NewTab => {
                self.show_shell_picker = true;
                self.shell_picker_selected = 0;
                Some(Task::none())
            }
            ShortcutAction::CloseTab => {
                self.close_active_target();
                Some(Task::none())
            }
            ShortcutAction::OpenSettings => {
                self.settings_open = true;
                self.active_tab = SETTINGS_TAB_INDEX;
                self.settings_draft = SettingsDraft::from_config(&self.config);
                Some(Task::none())
            }
            ShortcutAction::NextTab => {
                self.select_relative_tab(1);
                Some(Task::none())
            }
            ShortcutAction::PrevTab => {
                self.select_relative_tab(-1);
                Some(Task::none())
            }
            ShortcutAction::Quit => Some(iced::exit()),
        }
    }

    fn close_active_target(&mut self) {
        if self.active_tab == SETTINGS_TAB_INDEX {
            self.settings_open = false;
            self.active_tab = self.tabs.len().saturating_sub(1);
            return;
        }

        if self.tabs.is_empty() {
            return;
        }

        let index = self.active_tab.min(self.tabs.len() - 1);
        self.tabs.remove(index);

        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
            self.active_tab = self.tabs.len() - 1;
        }
        if self.tabs.is_empty() {
            self.active_tab = 0;
        }
    }

    fn select_relative_tab(&mut self, step: isize) {
        let mut visible_tabs: Vec<usize> = (0..self.tabs.len()).collect();
        if self.settings_open {
            visible_tabs.push(SETTINGS_TAB_INDEX);
        }

        if visible_tabs.is_empty() {
            return;
        }

        let current_pos = visible_tabs
            .iter()
            .position(|index| *index == self.active_tab)
            .unwrap_or(0);

        let len = visible_tabs.len() as isize;
        let next_pos = (current_pos as isize + step).rem_euclid(len) as usize;
        self.active_tab = visible_tabs[next_pos];
    }
}
