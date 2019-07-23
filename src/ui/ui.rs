use std::env;
use std::io;
use std::path::Path;

use termion::event::Key;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, Paragraph, SelectableList, Text, Widget};
use tui::Terminal;

use super::util::event::{Event, Events};
use crate::EntryWrapper;
use crate::GenericError;

const EXIT_COMMAND: &'static str = "exit";

struct UIEntry {
    entry: Option<EntryWrapper>,
    selected: Option<usize>,
    file_style: Style,
    dir_style: Style,
}

impl UIEntry {
    fn new_empty() -> UIEntry {
        UIEntry {
            entry: None,
            selected: None,
            file_style: Style::default(),
            dir_style: Style::default().fg(Color::Blue),
        }
    }

    fn new<P: AsRef<Path> + std::convert::AsRef<std::ffi::OsStr>>(
        entry_path: P,
    ) -> Result<UIEntry, GenericError> {
        let entry = EntryWrapper::new_with_memstorage(entry_path)?;
        Ok(UIEntry {
            entry: Some(entry),
            selected: None,
            file_style: Style::default(),
            dir_style: Style::default().fg(Color::Blue),
        })
    }

    fn replace_entry<P: AsRef<Path> + std::convert::AsRef<std::ffi::OsStr>>(
        &mut self,
        entry_path: P,
    ) -> Result<(), GenericError> {
        let entry = EntryWrapper::new_with_memstorage(entry_path)?;
        self.entry = Some(entry);
        Ok(())
    }

    fn replace_entry_with_parent(&mut self) {
        if let Some(entry) = &mut self.entry {
            if let Some(parent) = entry.get_parent() {
                entry.replace_from_storage(&parent);
                self.selected = None;
            }
        }
    }

    fn replace_entry_at_selected(&mut self) {
        if let Some(selected) = self.selected {
            let children = self.get_children();
            let selected_entry = children.get(selected);
            if let Some(selected_entry) = selected_entry {
                if let Some(entry) = &mut self.entry {
                    entry.replace_from_storage(&selected_entry);
                    self.selected = None;
                }
            }
        }
    }

    fn load_entry(&mut self) -> bool {
        let e = &mut self.entry;
        match e {
            Some(e) => {
                e.load_all_children();
                true
            }
            None => false,
        }
    }

    fn get_children(&self) -> Vec<String> {
        match &self.entry {
            Some(entry) => entry.get_children(),
            None => vec![],
        }
    }

    fn get_number_children(&self) -> usize {
        match &self.entry {
            Some(entry) => entry.get_children_len(),
            None => 0,
        }
    }

    fn get_name(&self) -> String {
        match &self.entry {
            Some(entry) => entry.get_name(),
            None => "".to_string(),
        }
    }

    // TODO: implement
    fn get_metadata(&self) -> Vec<String> {
        vec![]
        // match &self.entry {
        // None => vec![],
        // Some(entry) => {
        // // let entry = entry
        // let metadata = entry.get_metadata();
        // vec![
        // format!("size : {}", metadata.get_size()),
        // format!(
        // "last access time : {}",
        // match metadata.get_last_access_time() {
        // Some(value) => "",
        // None => "",
        // }
        // ),
        // format!(
        // "last modified time : {}",
        // match metadata.get_last_modified_time() {
        // Some(value) => "",
        // None => "",
        // }
        // ),
        // format!(
        // "creation time : {}",
        // match metadata.get_creation_time() {
        // Some(value) => "",
        // None => "",
        // }
        // ),
        // ]
        // }
        // }
    }
}

pub fn run() -> Result<(), GenericError> {
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;
    terminal.clear()?;

    let events = Events::new();
    let mut command = String::new();

    let mut main_entry = UIEntry::new_empty();
    main_entry.replace_entry(env::current_dir().unwrap().to_str().unwrap())?;
    main_entry.load_entry();

    loop {
        terminal.draw(|mut f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(10),
                        Constraint::Percentage(80),
                        Constraint::Percentage(10),
                    ]
                    .as_ref(),
                )
                .split(f.size());
            let title_style = Style::default().modifier(Modifier::BOLD);
            // Title area
            {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(100)].as_ref())
                    .split(chunks[0]);
                let text = [Text::raw(main_entry.get_name())];
                Paragraph::new(text.iter())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Title")
                            .title_style(title_style),
                    )
                    .alignment(Alignment::Left)
                    .wrap(true)
                    .render(&mut f, chunks[0]);
            }
            // Main area
            {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                    .split(chunks[1]);

                // TODO: build up entries with specific style for file and dir
                // let entries = main_entry.get_entries().iter().map(|e| {});
                SelectableList::default()
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Entries")
                            .title_style(title_style),
                    )
                    .items(&main_entry.get_children())
                    .select(main_entry.selected)
                    .highlight_style(Style::default().modifier(Modifier::BOLD))
                    .highlight_symbol(">")
                    .render(&mut f, chunks[0]);
                List::new(
                    main_entry
                        .get_metadata()
                        .iter()
                        .map(|e| Text::styled(e, main_entry.file_style)),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Info")
                        .title_style(title_style),
                )
                .render(&mut f, chunks[1]);
            }
            // Cmd area
            {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(100)].as_ref())
                    .split(chunks[2]);
                let text = [Text::raw(command.as_str())];
                Paragraph::new(text.iter())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("CMD")
                            .title_style(title_style),
                    )
                    .alignment(Alignment::Left)
                    .wrap(true)
                    .render(&mut f, chunks[0]);
            }
        })?;

        match events.next()? {
            Event::Input(input) => match input {
                Key::Char(ch) => {
                    if ch == '\n' {
                        let exit = parse_command(command.trim());
                        if exit {
                            break;
                        }
                        command.clear();
                    } else {
                        command.push(ch);
                    }
                }
                Key::Backspace => {
                    command.pop();
                }
                Key::Left => main_entry.replace_entry_with_parent(),
                Key::Right => main_entry.replace_entry_at_selected(),
                Key::Down => {
                    main_entry.selected = if let Some(selected) = main_entry.selected {
                        if selected >= main_entry.get_number_children() - 1 {
                            Some(0)
                        } else {
                            Some(selected + 1)
                        }
                    } else {
                        Some(0)
                    }
                }
                Key::Up => {
                    main_entry.selected = if let Some(selected) = main_entry.selected {
                        if selected > 0 {
                            Some(selected - 1)
                        } else {
                            Some(main_entry.get_number_children() - 1)
                        }
                    } else {
                        Some(0)
                    }
                }
                _ => (),
            },
            _ => (),
        }
    }

    Ok(())
}

// TODO: more commands?
fn parse_command(command: &str) -> bool {
    match command {
        EXIT_COMMAND => true,
        _ => false,
    }
}
