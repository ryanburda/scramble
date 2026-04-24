use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use rand::seq::SliceRandom;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

const ROWS: usize = 5;
const COLS: usize = 5;

/// Primary key layout for each grid position (shown on tiles).
/// Row 0: y u i o p
/// Row 1: h j k l ;
/// Row 2: n m , . /
/// Row 3: a s d f g
/// Row 4: z x c v b
const KEY_LABELS: [[char; COLS]; ROWS] = [
    ['y', 'u', 'i', 'o', 'p'],
    ['h', 'j', 'k', 'l', ';'],
    ['n', 'm', ',', '.', '/'],
    ['a', 's', 'd', 'f', 'g'],
    ['z', 'x', 'c', 'v', 'b'],
];

/// Alternate key layout for row 2.
/// Row 2 alt: q w e r t
const ALT_ROW2: [char; COLS] = ['q', 'w', 'e', 'r', 't'];

const ALL_COLORS: [TileColor; 6] = [
    TileColor::Red,
    TileColor::Blue,
    TileColor::Green,
    TileColor::Yellow,
    TileColor::White,
    TileColor::Orange,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tile {
    Color(TileColor),
    Empty,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TileColor {
    Red,
    Blue,
    Green,
    Yellow,
    White,
    Orange,
}

impl TileColor {
    fn to_ratatui_color(self) -> Color {
        match self {
            TileColor::Red => Color::Red,
            TileColor::Blue => Color::Blue,
            TileColor::Green => Color::Green,
            TileColor::Yellow => Color::Yellow,
            TileColor::White => Color::White,
            TileColor::Orange => Color::Rgb(255, 165, 0),
        }
    }

}

struct Game {
    board: [[Tile; COLS]; ROWS],
    goal: [[TileColor; 3]; 3],
    empty: (usize, usize),
    moves: u32,
    solved: bool,
    started_at: Instant,
    solve_time: Option<Duration>,
}

impl Game {
    fn new() -> Self {
        let mut rng = rand::rng();

        // Generate a random 3x3 goal from dice (each cell is a random color).
        let mut goal = [[TileColor::Red; 3]; 3];
        for row in goal.iter_mut() {
            for cell in row.iter_mut() {
                *cell = ALL_COLORS[rand::random_range(0..ALL_COLORS.len())];
            }
        }

        // 24 colored tiles + 1 empty = 25 (5x5).
        // 4 of each of 6 colors = 24 tiles.
        let mut tiles: Vec<Tile> = Vec::with_capacity(25);
        for &color in &ALL_COLORS {
            for _ in 0..4 {
                tiles.push(Tile::Color(color));
            }
        }
        tiles.push(Tile::Empty);
        tiles.shuffle(&mut rng);

        let mut board = [[Tile::Empty; COLS]; ROWS];
        let mut empty = (0, 0);
        for (i, tile) in tiles.iter().enumerate() {
            let r = i / COLS;
            let c = i % COLS;
            board[r][c] = *tile;
            if *tile == Tile::Empty {
                empty = (r, c);
            }
        }

        let mut game = Game {
            board,
            goal,
            empty,
            moves: 0,
            solved: false,
            started_at: Instant::now(),
            solve_time: None,
        };
        game.check_solved();
        game
    }

    fn key_to_pos(key: char) -> Option<(usize, usize)> {
        // Check primary layout
        for (r, row) in KEY_LABELS.iter().enumerate() {
            for (c, &k) in row.iter().enumerate() {
                if k == key {
                    return Some((r, c));
                }
            }
        }
        // Check alternate row 2
        for (c, &k) in ALT_ROW2.iter().enumerate() {
            if k == key {
                return Some((2, c));
            }
        }
        None
    }

    fn slide(&mut self, pos: (usize, usize)) {
        let (pr, pc) = pos;
        let (er, ec) = self.empty;

        // Must be on the same row or same column as the empty space, and not the empty space itself.
        if pos == self.empty {
            return;
        }

        if pr == er {
            // Same row: shift tiles horizontally toward the empty space.
            if pc < ec {
                // Pressed tile is left of empty — shift right.
                for c in (pc..ec).rev() {
                    self.board[er][c + 1] = self.board[er][c];
                }
            } else {
                // Pressed tile is right of empty — shift left.
                for c in (ec + 1)..=pc {
                    self.board[er][c - 1] = self.board[er][c];
                }
            }
        } else if pc == ec {
            // Same column: shift tiles vertically toward the empty space.
            if pr < er {
                // Pressed tile is above empty — shift down.
                for r in (pr..er).rev() {
                    self.board[r + 1][ec] = self.board[r][ec];
                }
            } else {
                // Pressed tile is below empty — shift up.
                for r in (er + 1)..=pr {
                    self.board[r - 1][ec] = self.board[r][ec];
                }
            }
        } else {
            // Not aligned with the empty space.
            return;
        }

        self.board[pr][pc] = Tile::Empty;
        self.empty = (pr, pc);
        self.moves += 1;
        self.check_solved();
    }

    fn check_solved(&mut self) {
        for r in 0..3 {
            for c in 0..3 {
                match self.board[r + 1][c + 1] {
                    Tile::Color(tc) if tc == self.goal[r][c] => {}
                    _ => {
                        self.solved = false;
                        return;
                    }
                }
            }
        }
        self.solved = true;
        if self.solve_time.is_none() {
            self.solve_time = Some(self.started_at.elapsed());
        }
    }

    fn elapsed(&self) -> Duration {
        self.solve_time.unwrap_or_else(|| self.started_at.elapsed())
    }

    fn handle_key(&mut self, c: char) {
        if let Some(pos) = Self::key_to_pos(c) {
            self.slide(pos);
        }
    }
}

fn render_tile_cell(tile: Tile, key: char, in_goal_zone: bool) -> Vec<Line<'static>> {
    match tile {
        Tile::Color(color) => {
            let bg = color.to_ratatui_color();
            let fg = match color {
                TileColor::White | TileColor::Yellow => Color::Black,
                _ => Color::White,
            };
            let style = Style::default().fg(fg).bg(bg);
            vec![
                Line::from(Span::styled(format!(" {:<6}", key), style)),
                Line::from(Span::styled(format!("{:7}", ""), style)),
                Line::from(Span::styled(format!("{:7}", ""), style)),
            ]
        }
        Tile::Empty => {
            let fg = if in_goal_zone {
                Color::White
            } else {
                Color::DarkGray
            };
            let style = Style::default().fg(fg);
            vec![
                Line::from(Span::styled(format!(" {:<6}", key), style)),
                Line::from(Span::raw("")),
                Line::from(Span::raw("")),
            ]
        }
    }
}

fn render_goal_cell(color: TileColor) -> Vec<Line<'static>> {
    let bg = color.to_ratatui_color();
    let fg = match color {
        TileColor::White | TileColor::Yellow => Color::Black,
        _ => Color::White,
    };
    let style = Style::default().fg(fg).bg(bg);
    vec![
        Line::from(Span::styled(format!("{:5}", ""), style)),
        Line::from(Span::styled(format!("{:5}", ""), style)),
        Line::from(Span::styled(format!("{:5}", ""), style)),
    ]
}

fn draw(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, game: &Game) -> io::Result<()> {
    terminal.draw(|frame| {
        let area = frame.area();

        let tile_w = 7u16;
        let tile_h = 3u16;
        let grid_w = tile_w * COLS as u16 + 2;
        let grid_h = tile_h * ROWS as u16 + 2;

        let goal_tile_w = 5u16;
        let goal_grid_w = goal_tile_w * 3 + 2;
        let goal_grid_h = tile_h * 3 + 2;

        let gap = 4u16;
        let total_w = grid_w + gap + goal_grid_w;

        // Center everything
        let x = area.x + area.width.saturating_sub(total_w) / 2;
        let y = area.y + area.height.saturating_sub(grid_h + 4) / 2;

        // Title
        let elapsed = game.elapsed();
        let secs = elapsed.as_secs();
        let tenths = elapsed.subsec_millis() / 100;
        let time_str = format!("{:02}:{:02}.{}", secs / 60, secs % 60, tenths);
        let status = if game.solved { "  SOLVED!" } else { "" };
        let title = Paragraph::new(Line::from(vec![
            Span::styled("SCRAMBLE", Style::default().fg(Color::Cyan)),
            Span::raw(format!("  moves: {}  time: {}", game.moves, time_str)),
            Span::styled(status, Style::default().fg(Color::Green)),
        ]));
        frame.render_widget(title, Rect::new(x, y, total_w + 10, 1));

        // === Board grid ===
        let grid_rect = Rect::new(x, y + 2, grid_w, grid_h);
        let board_border_color = if game.solved {
            Color::Green
        } else {
            Color::DarkGray
        };
        let border = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(board_border_color));
        frame.render_widget(border, grid_rect);

        let inner_x = grid_rect.x + 1;
        let inner_y = grid_rect.y + 1;

        let row_constraints: Vec<Constraint> =
            (0..ROWS).map(|_| Constraint::Length(tile_h)).collect();
        let col_constraints: Vec<Constraint> =
            (0..COLS).map(|_| Constraint::Length(tile_w)).collect();

        let inner_rect = Rect::new(
            inner_x,
            inner_y,
            tile_w * COLS as u16,
            tile_h * ROWS as u16,
        );
        let rows = Layout::vertical(row_constraints).split(inner_rect);

        for (r, row_area) in rows.iter().enumerate() {
            let cols = Layout::horizontal(col_constraints.clone()).split(*row_area);
            for (c, cell_area) in cols.iter().enumerate() {
                let tile = game.board[r][c];
                let key = KEY_LABELS[r][c];
                let in_goal_zone = r >= 1 && r <= 3 && c >= 1 && c <= 3;
                let lines = render_tile_cell(tile, key, in_goal_zone);
                frame.render_widget(Paragraph::new(lines), *cell_area);
            }
        }

        // === Goal grid ===
        let goal_x = x + grid_w + gap;
        // Vertically center the goal next to the board
        let goal_y = y + 2 + (grid_h.saturating_sub(goal_grid_h + 2)) / 2;

        let goal_label = Paragraph::new(Line::from(Span::styled(
            "GOAL",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(goal_label, Rect::new(goal_x, goal_y, goal_grid_w, 1));

        let goal_rect = Rect::new(goal_x, goal_y + 1, goal_grid_w, goal_grid_h);
        let goal_border = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        frame.render_widget(goal_border, goal_rect);

        let goal_inner_x = goal_rect.x + 1;
        let goal_inner_y = goal_rect.y + 1;

        let goal_row_constraints: Vec<Constraint> =
            (0..3).map(|_| Constraint::Length(tile_h)).collect();
        let goal_col_constraints: Vec<Constraint> =
            (0..3).map(|_| Constraint::Length(goal_tile_w)).collect();

        let goal_inner_rect = Rect::new(
            goal_inner_x,
            goal_inner_y,
            goal_tile_w * 3,
            tile_h * 3,
        );
        let goal_rows = Layout::vertical(goal_row_constraints).split(goal_inner_rect);

        for (r, row_area) in goal_rows.iter().enumerate() {
            let cols = Layout::horizontal(goal_col_constraints.clone()).split(*row_area);
            for (c, cell_area) in cols.iter().enumerate() {
                let lines = render_goal_cell(game.goal[r][c]);
                frame.render_widget(Paragraph::new(lines), *cell_area);
            }
        }

        // Help text
        let help_y = y + 2 + grid_h + 1;
        let help = Paragraph::new(Line::from(vec![
            Span::styled(
                "Slide tiles to match the inner 3x3 to the goal. ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("N", Style::default().fg(Color::Yellow)),
            Span::styled(": new  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::styled(": quit", Style::default().fg(Color::DarkGray)),
        ]));
        frame.render_widget(help, Rect::new(x, help_y, total_w + 10, 1));
    })?;
    Ok(())
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut game = Game::new();

    loop {
        draw(&mut terminal, &game)?;

        // Poll with timeout so the timer redraws even without input.
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Esc => break,
                    KeyCode::Char('N') => game = Game::new(),
                    KeyCode::Char(c) => {
                        if !game.solved {
                            game.handle_key(c);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
