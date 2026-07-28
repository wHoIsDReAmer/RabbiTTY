use super::super::command_palette::{CommandEntry, CommandTarget, filter, wrap};
use super::super::{App, Message};
use crate::config::ShortcutId;
use iced::Task;

impl App {
    pub(in crate::gui) fn command_entries(&self) -> Vec<CommandEntry> {
        let mut entries: Vec<CommandEntry> = ShortcutId::ALL
            .into_iter()
            .filter(|id| *id != ShortcutId::CommandPalette)
            .map(|id| CommandEntry {
                label: id.label().to_string(),
                detail: self.config.shortcuts.get(id).to_string(),
                target: CommandTarget::Builtin(id),
            })
            .collect();

        if let Some(registry) = self.plugins.as_ref() {
            entries.extend(registry.contributed_commands().into_iter().map(|command| {
                CommandEntry {
                    label: command.title,
                    detail: command.source,
                    target: CommandTarget::Plugin {
                        plugin: command.plugin,
                        command: command.id,
                    },
                }
            }));
        }
        entries
    }

    pub(in crate::gui) fn visible_command_entries(&self) -> Vec<CommandEntry> {
        filter(self.command_entries(), &self.command_query)
    }

    pub(in crate::gui) fn open_command_palette(&mut self) -> Task<Message> {
        self.show_command_palette = true;
        self.command_query.clear();
        self.command_selected = 0;
        iced::widget::operation::focus(COMMAND_INPUT_ID.clone())
    }

    pub(in crate::gui) fn overlay_owns_keyboard(&self) -> bool {
        self.show_command_palette || self.password_prompt.is_some()
    }

    pub(in crate::gui) fn close_command_palette(&mut self) {
        self.show_command_palette = false;
        self.command_query.clear();
        self.command_selected = 0;
    }

    pub(in crate::gui) fn shift_command_selection(&mut self, delta: i32) {
        let len = self.visible_command_entries().len();
        self.command_selected = wrap(self.command_selected, delta, len);
    }

    pub(in crate::gui) fn run_command_entry(&mut self, index: usize) -> Task<Message> {
        let Some(entry) = self.visible_command_entries().into_iter().nth(index) else {
            return Task::none();
        };
        self.close_command_palette();

        match entry.target {
            CommandTarget::Builtin(id) => self.run_builtin(id),
            CommandTarget::Plugin { plugin, command } => {
                self.run_plugin_command(&plugin, &command);
                Task::none()
            }
        }
    }

    fn run_builtin(&mut self, id: ShortcutId) -> Task<Message> {
        use crate::gui::app::shortcuts::ShortcutAction;
        self.apply_shortcut_action(ShortcutAction::from_id(id))
    }
}

pub(in crate::gui) static COMMAND_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(iced::widget::Id::unique);
