mod hw_enum;
mod ui;
mod burn_logic;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind}, 
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}
};
use ratatui::{Terminal, backend, prelude::CrosstermBackend};
use std::{error::Error, io, time::Duration};
use rfd::FileDialog;
use std::sync::mpsc::{self, Receiver};
use crate::{burn_logic::spawn_burn_thread, ui::CurrentScreen, burn_logic::BurnEvent};

//Combine these functions into one later, they are basically the same
fn file_dlg() -> Vec<String> {
    disable_raw_mode().expect("could not disable raw mode");
    let file_task = std::thread::spawn(move || {
        FileDialog::new().add_filter("All Files", &["*"]).pick_files()
    });

    let mut paths = Vec::new();
    if let Ok(Some(files)) = file_task.join() {
        for path in files {
            if let Some(path_str) = path.to_str() {paths.push(path_str.to_string());}
        }
    }
    enable_raw_mode().expect("Could not enable raw mode");
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture).expect("Could not run execute macro");
    return paths;
}

fn folder_dlg() -> Vec<String> {
    disable_raw_mode().expect("could not disable raw mode");
    let folder_task = std::thread::spawn(move || {
        FileDialog::new().pick_folder()
    });

    let mut paths = Vec::new();
    if let Ok(Some(folder)) = folder_task.join() {
        if let Some(path_str) = folder.to_str() {paths.push(path_str.to_string());}
    }
    enable_raw_mode().expect("Could not enable raw mode");
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture).expect("Could not run execute macro");
    return paths;
}


fn main() -> Result<(), Box<dyn Error>>{
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).expect("go away yellow lines");
    let backend = CrosstermBackend::new(stdout);
    let mut terminal  = Terminal::new(backend)?;

    let drive_handle = std::thread::spawn(|| {
        hw_enum::list_drives().expect("drive enumeration failed")
    });

    let mut app = ui::App::new();
    while !drive_handle.is_finished() {
        terminal.draw(|f| ui::draw(f, &mut app))?;
        app.tick();
        std::thread::sleep(Duration::from_millis(100));
    }
    app.drives = drive_handle.join().expect("drive enumeration thread panicked");
    app.current_screen = CurrentScreen::Dashboard;
    if app.drives.len() > 0 {app.drive_state.select(Some(0));}

    loop {

        while let Ok(event) = app.reciever.as_ref().unwrap_or(&mpsc::channel().1).try_recv() {
            match event {
                BurnEvent::Progress(p) => {app.prog_ratio = p;}
                BurnEvent::Log(s) => {app.logs.push(s);}
                BurnEvent::Error(e) => {
                    app.logs.push(format!("Error: {}", e));
                }
                BurnEvent::Finished => {
                    app.logs.push("Burn Finished".to_string());
                    app.logs.push("press any key to continue".to_string());
                }
            }
        }
        

        terminal.draw(|f| ui::draw(f, &mut app))?;

        if event::poll(Duration::from_millis(100))? {

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {continue;}
                match app.current_screen {
                    ui::CurrentScreen::Help => {
                        if let KeyCode::Esc = key.code {app.current_screen = CurrentScreen::Dashboard}
                    }

                    ui::CurrentScreen::ActiveBurn => {
                        if let KeyCode::Esc = key.code {app.current_screen = CurrentScreen::Dashboard}
                    }

                    ui::CurrentScreen::Dashboard => {
                        match app.input_mode {
                            true => match key.code {
                                KeyCode::Esc => app.input_mode = false,
                                KeyCode::Char(c) => {app.input_buffer.push(c);}
                                KeyCode::Backspace => {app.input_buffer.pop();}
                                KeyCode::Enter => {
                                    let command = app.input_buffer.trim().to_lowercase();
                                    match command.as_str() {
                                        "add" => {
                                            app.files.extend(file_dlg());
                                            if app.files.len() > 0 {app.file_state.select(Some(0));}
                                            terminal.clear()?;
                                            
                                        }
                                        "addfolder" => {
                                            app.files.extend(folder_dlg());
                                            if app.files.len() > 0 {app.file_state.select(Some(0));}
                                            terminal.clear()?;
                                        }
                                        "help" => {app.current_screen = CurrentScreen::Help}
                                        "quit" => { break; }
                                        "exit" => { break; }
                                        "setfinalize" => {app.finalize = !app.finalize}
                                        "burn" => {
                                            let (tx, rx) = mpsc::channel();
                                            app.reciever = Some(rx);
                                            app.current_screen = CurrentScreen::ActiveBurn;
                                            app.logs.clear();
                                            //handle None state more gracefully later
                                            //this is bad
                                            let Some(index) = app.drive_state.selected() else {break};
                                            let drive_id = &app.drives[index].id;
                                            spawn_burn_thread(&app.files, &app.volume_label, &drive_id, tx);
                                        }
                                        _ => {}
                                    }
                                    if command.contains("setvl") && command.len() > 5 {app.volume_label = command[5..].to_string();}
                                    if command.contains("setspeed") && command.len() > 8 {
                                        app.burn_speed = format!("{}x", command.chars().nth(9).expect("Please enter a number after the command"));
                                    }
                                    // handle commands here later
                                    app.input_buffer.clear();
                                }
                                _ => {}
                            }

                            false => match key.code {
                                KeyCode::Char('q') => break,
                                KeyCode::Char('i') => app.input_mode = true,
                                KeyCode::Char('h') => app.current_screen = ui::CurrentScreen::Help,
                                KeyCode::Char('r') => app.remove(),
                                KeyCode::Char('a') => {
                                    app.files.extend(file_dlg());
                                    if app.files.len() > 0 {app.file_state.select(Some(0));}
                                    terminal.clear()?;
                                }
                                KeyCode::Tab => app.current_menu = app.current_menu.next(),
                                KeyCode::BackTab => app.current_menu = app.current_menu.prev(),
                                KeyCode::Down => app.donkey(),
                                KeyCode::Up => app.upkey(),
                                KeyCode::Enter => app.select(),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                    
                }
                
            }
        }
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}