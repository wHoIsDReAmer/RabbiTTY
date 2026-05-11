use super::super::{App, Message};
use crate::config::AppConfigUpdates;
use crate::gui::settings::SettingsDraft;
use crate::terminal::TerminalTheme;
use iced::{Size, Task, window};

impl App {
    pub(super) fn apply_settings(&mut self, save: bool) -> Task<Message> {
        let updates = self.settings_draft.to_updates();

        #[cfg(target_os = "macos")]
        {
            let blur_toggled = updates
                .blur_enabled
                .is_some_and(|v| v != self.config.theme.blur_enabled);
            let radius_changed = updates
                .macos_blur_radius
                .is_some_and(|v| v != self.config.theme.macos_blur_radius);
            if blur_toggled || radius_changed {
                self.show_restart_confirm = true;
                self.pending_settings_updates = Some(updates);
                self.pending_save_on_restart = save;
                return Task::none();
            }
        }

        let resize_task = self.apply_updates_to_runtime(updates);

        if save {
            self.queue_config_save();
        }

        resize_task
    }

    pub(super) fn queue_config_save(&self) {
        let _ = self.config_save_tx.send(self.config.clone());
    }

    pub(super) fn apply_updates_to_runtime(&mut self, updates: AppConfigUpdates) -> Task<Message> {
        self.config.apply_updates(updates);
        crate::i18n::set_locale(self.config.ui.language.as_deref());
        self.palette = crate::gui::theme::Palette::from_theme(&self.config.theme);
        self.settings_draft = SettingsDraft::from_config(&self.config);

        let new_size = Size::new(self.config.ui.window_width, self.config.ui.window_height);
        let resize_task = if (self.window_size.width - new_size.width).abs() > f32::EPSILON
            || (self.window_size.height - new_size.height).abs() > f32::EPSILON
        {
            self.window_size = new_size;
            window::latest().and_then(move |id| window::resize(id, new_size))
        } else {
            Task::none()
        };

        let (cols, rows) = self.grid_for_size(self.window_size);
        let theme = TerminalTheme::from_config(&self.config);
        for tab in &mut self.tabs {
            tab.resize(cols, rows);
            tab.set_theme(theme.clone());
        }

        resize_task
    }

    #[cfg(target_os = "macos")]
    pub(super) fn handle_confirm_restart(&mut self) -> Task<Message> {
        if let Some(updates) = self.pending_settings_updates.take() {
            let _ = self.apply_updates_to_runtime(updates);
            if self.pending_save_on_restart
                && let Err(err) = self.config.save()
            {
                eprintln!("Failed to save config: {err}");
            }
        }

        let restart_spawned = match std::env::current_exe() {
            Ok(current_exe) => {
                let args: Vec<_> = std::env::args_os().skip(1).collect();
                match std::process::Command::new(current_exe).args(args).spawn() {
                    Ok(_) => true,
                    Err(err) => {
                        eprintln!("Failed to relaunch app: {err}");
                        false
                    }
                }
            }
            Err(err) => {
                eprintln!("Failed to locate executable for restart: {err}");
                false
            }
        };

        self.show_restart_confirm = false;
        self.pending_save_on_restart = false;

        if restart_spawned {
            return iced::exit();
        }

        Task::none()
    }
}
