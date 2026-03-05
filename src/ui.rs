use ratatui::widgets::ListState;
use ratatui::{prelude::*, widgets::*};
use throbber_widgets_tui::{ThrobberState, Throbber, BRAILLE_SIX_DOUBLE};
use std::sync::mpsc::{self, Receiver};
use crate::hw_enum::DriveInfo;
use crate::burn_logic::{spawn_burn_thread, BurnEvent};


#[derive(PartialEq)]
pub enum CurrentScreen {
    Dashboard,
    Help,
    ActiveBurn,
    Splash,
}
#[derive(PartialEq, Clone, Copy)]
pub enum CurrentMenu {
    Media,
    Files,
    Status,
}
impl CurrentMenu {
    pub fn next(&self) -> Self {
        match self {
            CurrentMenu::Files => CurrentMenu::Status,
            CurrentMenu::Status => CurrentMenu::Media,
            CurrentMenu::Media => CurrentMenu::Files
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            CurrentMenu::Files => CurrentMenu::Media,
            CurrentMenu::Status => CurrentMenu::Files,
            CurrentMenu::Media => CurrentMenu::Status
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum CurrentOption{
    VolumeLabel,
    Speed,
    Finalize,
    Burn,
}
impl CurrentOption {
    pub fn next(&self) -> Self {
        match self {
            CurrentOption::VolumeLabel => CurrentOption::Speed,
            CurrentOption::Speed => CurrentOption::Finalize,
            CurrentOption::Finalize => CurrentOption::Burn,
            CurrentOption::Burn => CurrentOption::VolumeLabel
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            CurrentOption::VolumeLabel => CurrentOption::Burn,
            CurrentOption::Speed => CurrentOption::VolumeLabel,
            CurrentOption::Finalize => CurrentOption::Speed,
            CurrentOption::Burn => CurrentOption::Finalize,
        }
    }
}


pub struct App {
    pub reciever: Option<Receiver<BurnEvent>>,
    pub prog_ratio: f32,
    pub logs: Vec<String>,
    pub drives: Vec<DriveInfo>,
    pub drive_state: ListState,
    pub throbber_state: ThrobberState,

    pub files: Vec<String>,
    pub selected_files: Vec<String>,
    pub file_state: ListState,

    pub volume_label: String,
    pub burn_speed: String,
    pub finalize: bool,

    pub input_buffer: String,
    pub input_mode: bool,

    pub current_screen: CurrentScreen,
    pub current_menu: CurrentMenu,
    pub current_option: CurrentOption,

}

impl App{
    pub fn new() -> App {
        App{
            drives : Vec::new(),
            drive_state: ListState::default(),
            files: Vec::new(),
            selected_files: Vec::new(),
            file_state: ListState::default(),
            volume_label: String::from("NEW DISK"),
            burn_speed: String::from("8x"),
            finalize: true,
            input_buffer: String::new(),
            input_mode: true,
            throbber_state: ThrobberState::default(),
            current_screen: CurrentScreen::Splash,
            current_menu: CurrentMenu::Files,
            current_option: CurrentOption::VolumeLabel,
            logs: Vec::new(),
            prog_ratio: 0.0,
            reciever: None,
        }
    }

    pub fn tick(&mut self){
        self.throbber_state.calc_next();
    }

    pub fn set_color(&self, current: CurrentMenu, target: CurrentMenu) -> Color {
        if current == target && self.input_mode == false {Color::Blue}
        else {Color::White}
    }

    pub fn set_bg(&self, current: CurrentOption, target: CurrentOption) -> Color {
        if current == target && self.input_mode == false && self.current_menu == CurrentMenu::Status {Color::Blue}
        else {Color::Reset}
    }

    pub fn get_highlight_style(&self, target: CurrentMenu) -> Style {
        if self.current_menu == target {Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)}
        else {Style::default().bg(Color::Reset).fg(Color::Blue)}
    }

    pub fn upkey(&mut self) {
        match self.current_menu {
            CurrentMenu::Media => {
                let i =  match self.drive_state.selected() {
                    Some(i) => if i == 0 {self.drives.len() - 1} else {i - 1},
                    None => 0,
                };
                self.drive_state.select(Some(i));
            }
            CurrentMenu::Files => {
                let i = match self.file_state.selected() {
                    Some(i) => if i == 0 {self.files.len() - 1 } else {i - 1},
                    None => 0,
                };
                self.file_state.select(Some(i));
            }
            CurrentMenu::Status => {
                self.current_option = self.current_option.prev();
            }
        }
    }

    pub fn donkey(&mut self) {
        match self.current_menu {
            CurrentMenu::Media => {
                let i = match self.drive_state.selected() {
                    Some(i) => if i >= self.drives.len() - 1 {0} else {i + 1}
                    None => 0,
                };
                self.drive_state.select(Some(i));
            }
            CurrentMenu::Files => {
                let i = match self.file_state.selected() {
                    Some(i) => if i >= self.files.len() - 1 {0} else {i + 1}
                    None => 0,
                };
                self.file_state.select(Some(i));
            }
            CurrentMenu::Status => {
                self.current_option = self.current_option.next();
            }
        }
    }

    pub fn select(&mut self) {
        let speed_list = vec!["4x", "8x", "16x", "24x"];
        match self.current_menu {
            CurrentMenu::Media => {

            }
            CurrentMenu::Files => {
                let i = match self.file_state.selected() {
                    Some(i) => self.selected_files.push(i.to_string()),
                    None => (),
                };
            }
            CurrentMenu::Status => {
                match self.current_option {
                    CurrentOption::VolumeLabel => {}
                    CurrentOption::Finalize => {self.finalize = !self.finalize}
                    CurrentOption::Speed => {
                        if let Some(current_index) = speed_list.iter().position(|&x| x == self.burn_speed) {
                            let next_index = (current_index + 1) % speed_list.len();
                            self.burn_speed = speed_list[next_index].to_string();
                        }
                        
                    }
                    CurrentOption::Burn => {
                        let (tx, rx) = mpsc::channel();
                        self.reciever = Some(rx);
                        let Some(drive_index) = self.drive_state.selected() else {return;};
                        let drive_id = &self.drives[drive_index].id;
                        spawn_burn_thread(&self.files,&self.volume_label, drive_id, tx, self.finalize);
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn remove(&mut self) {
        if let Some(selected_file) = self.file_state.selected() {
            self.files.remove(selected_file);
        }

    }


}


pub fn draw(f: &mut Frame, app: &mut App) {
    match app.current_screen {
        CurrentScreen::Dashboard => draw_dashboard(f, app),
        CurrentScreen::Help => draw_help(f),
        CurrentScreen::ActiveBurn => draw_burn(f, app),
        CurrentScreen::Splash => draw_init(f),
        _ => {}
        
    }
}

pub fn draw_init(f: &mut Frame){
    // Vertical centering
    let [_, center_row, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(2),
        Constraint::Fill(1),
    ]).areas(f.area());

    // Split into text line + throbber line
    let [text_area, throbber_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
    ]).areas(center_row);

    // Horizontal centering for throbber
    let [_, throbber_area, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(2),
        Constraint::Fill(1),
    ]).areas(throbber_row);

    let loading_text = Paragraph::new("Loading...")
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);
    f.render_widget(loading_text, text_area);

    let throbber = Throbber::default()
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .throbber_set(BRAILLE_SIX_DOUBLE);
    f.render_widget(throbber, throbber_area);
}

pub fn draw_help(f: &mut Frame) {
    let sections = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(3), Constraint::Min(1)]).split(f.area());
    let title = Paragraph::new("OXIDISK HELP").style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)).alignment(Alignment::Center);
    f.render_widget(title, sections[0]);

    let help_text = vec![
        Line::from(Span::styled("COMMAND MODE", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))).alignment(Alignment::Center),
        Line::from("  'q' : Exit Application."),
        Line::from("  'i' : Switch to input mode."),
        Line::from("  'a' : Add files"),
        Line::from("  'r' : Remove highlighted file"),
        Line::from("  'TAB' : Change active menu panel."),
        Line::from("  'SHIFT+TAB' : Change active menu panel in reverse"),
        Line::from("  'ENTER/RETURN' : Select highlighted option in active menu."),
        Line::from("  'UP/DOWN' : Change option in active menu."),
        Line::from(""),
        Line::from(Span::styled("INPUT MODE", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))).alignment(Alignment::Center),
        Line::from("  'quit' : Exit Application."),
        Line::from("  'add' : Adds file to burn list from a file dialog."),
        Line::from("  'remove <id>' : removes the file with the corresponding id from the burn list."),
        Line::from("  'setvl <volume label>' : Sets the volume label of the media to be burned."),
        Line::from("  'setspeed' <burn speed>' : Sets the burn speed (only takes integers: 8, 12, 16, 24"),
        Line::from("  'setfinalize' : Toggles finalizing of disk on and off"),
        Line::from("  'burn' : starts burning all files in the file list to the media in the selected drive"),       
        Line::from(Span::styled("Press 'q' or 'esc' to exit this page", Style::default().fg(Color::Blue))).alignment(Alignment::Center),
    ];

    let help_page = Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).padding(Padding::new(2, 2, 2, 1)));
    f.render_widget(help_page, sections[1]);

}

pub fn draw_burn(f: &mut Frame, app: &mut App){
    let rows = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1), Constraint::Max(5)]).split(f.area());

    //log box
    let log_items: Vec<ListItem> = app.logs.iter().map(|f| ListItem::new(f.as_str())).collect();
    let log_list: List = List::new(log_items).style(Style::default().fg(Color::White)).block(Block::default().borders(Borders::ALL).title("LOG"));
    f.render_widget(log_list, rows[0]);

    //progress bar
    let progress = Gauge::default().block(Block::default().borders(Borders::ALL).title("Progress")).gauge_style(Style::default().fg(Color::Green).bg(Color::Black)).percent((app.prog_ratio * 100.0) as u16);
    f.render_widget(progress, rows[1]);

}

pub fn draw_dashboard(f: &mut Frame, app: &mut App) {
    //global layout
    let rows = Layout::default().direction(Direction::Vertical).constraints([Constraint::Max(1), Constraint::Min(1), Constraint::Length(3)]).split(f.area());
    let columns = Layout::default().direction(Direction::Horizontal).constraints([
        Constraint::Percentage(20),
        Constraint::Percentage(55),
        Constraint::Percentage(25)
    ]).split(rows[1]);

    //title box
    let title = Paragraph::new("OXIDISK v0.1 | press 'h' for help or type 'help' in command line | press 'q' or type 'quit' to exit program").style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)).alignment(Alignment::Center);
    f.render_widget(title, rows[0]);

    //drive box 
    let drive_items: Vec<ListItem> = app.drives.iter().map(|d| {let label = format!("{} {} ", d.media_label, d.media_type.as_deref().unwrap_or("Unknown")); ListItem::new(label)}).collect();
    let drive_list = List::new(drive_items).style(Style::default().fg(app.set_color(app.current_menu, CurrentMenu::Media))).block(Block::default().borders(Borders::ALL).title("Drives"))
        .highlight_style(app.get_highlight_style(CurrentMenu::Media));

    f.render_stateful_widget(drive_list, columns[0], &mut app.drive_state);

    //file box 
    let file_items: Vec<ListItem> = app.files.iter().map(|f| ListItem::new(f.as_str())).collect();
    let file_list = List::new(file_items).style(Style::default().fg(app.set_color(app.current_menu, CurrentMenu::Files))).block(Block::default().borders(Borders::ALL).title("File List"))
        .highlight_style(app.get_highlight_style(CurrentMenu::Files));

    f.render_stateful_widget(file_list, columns[1], &mut app.file_state);

    //options box
    let options_text = vec![
        Line::from(vec![
            Span::raw("Volume Label: "),
            Span::styled(&app.volume_label, Style::default().fg(Color::Cyan).bg(app.set_bg(app.current_option, CurrentOption::VolumeLabel))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Burn Speed: "),
            Span::styled(&app.burn_speed, Style::default().fg(Color::Cyan).bg(app.set_bg(app.current_option, CurrentOption::Speed))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Finalize: "),
            Span::styled(app.finalize.to_string(), Style::default().fg(Color::Cyan).bg(app.set_bg(app.current_option, CurrentOption::Finalize))),
        ]),
        Line::from(""),
        Line::from(Span::styled("Burn", Style::default().fg(Color::Red).bg(app.set_bg(app.current_option, CurrentOption::Burn))))
    ];

    let options_box = Paragraph::new(options_text).style(Style::default().fg(app.set_color(app.current_menu, CurrentMenu::Status))).block(Block::default().borders(Borders::ALL).title("Options"));
    f.render_widget(options_box, columns[2]);

    //command box
    let input_color = if app.input_mode {Color::Green} else {Color::White};
    let input_box = Paragraph::new(app.input_buffer.to_string()).style(Style::default().fg(input_color)).block(Block::default().borders(Borders::ALL).title("Command")).alignment(Alignment::Left);
    f.render_widget(input_box, rows[2]);

    let cursor_x = rows[2].x + 1 + app.input_buffer.len() as u16;
    let cursor_y = rows[2].y + 1;
    if app.input_mode {f.set_cursor_position(Position::new(cursor_x, cursor_y));}
}

