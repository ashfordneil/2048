use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Figure out if the user is trying to escape the game, as raw mode stops all the usual suspects
/// from working
fn is_exit_request(key_event: KeyEvent) -> bool {
    if key_event.code == KeyCode::Esc {
        return true;
    }

    if key_event.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key_event.code, KeyCode::Char('c' | 'd'))
    {
        return true;
    }

    false
}

fn main() -> crossterm::Result<()> {
    let stdout = std::io::stdout();
    let mut rng = rand::thread_rng();

    let mut board = play_2048::Board::new();
    board.add_square(&mut rng);
    board.add_square(&mut rng);

    let mut renderer = play_2048::Renderer::new(stdout.lock())?;
    renderer.draw_board(&board)?;

    loop {
        match crossterm::event::read()? {
            Event::Key(evt) if is_exit_request(evt) => {
                break;
            }
            Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press,
                ..
            }) => {
                let action = match code {
                    KeyCode::Up | KeyCode::Char('w') => play_2048::Move::Up,
                    KeyCode::Down | KeyCode::Char('s') => play_2048::Move::Down,
                    KeyCode::Left | KeyCode::Char('a') => play_2048::Move::Left,
                    KeyCode::Right | KeyCode::Char('d') => play_2048::Move::Right,
                    _ => continue,
                };
                let new_board = board.apply_move(action);
                if new_board == board {
                    continue;
                }
                board = new_board;
                board.add_square(&mut rng);
                renderer.draw_board(&board)?;

                let alive = [
                    play_2048::Move::Up,
                    play_2048::Move::Down,
                    play_2048::Move::Left,
                    play_2048::Move::Right,
                ]
                .iter()
                .any(|&direction| board.apply_move(direction) != board);

                if !alive {
                    renderer.lose()?;
                    break;
                }
            }
            Event::Resize(columns, rows) => {
                renderer.resize((columns, rows))?;
                renderer.draw_board(&board)?;
            }
            _ => {}
        };
    }

    Ok(())
}
