fn main() {
    let ledger = tui_token_monitor::ledger::collect(chrono::Local::now());
    let view = tui_token_monitor::ui::ViewState {
        page: tui_token_monitor::ui::Page::Overview,
        snapshot: None,
        refreshing: false,
        last_error: None,
        age_secs: Some(3),
        ledger: Some(&ledger),
        period: tui_token_monitor::ui::overview::Period::Today,
        sel_agent: None, sel_day: None, sel_trend: None, sel_heat: None,
        chart_mode: true,
        trend_range: tui_token_monitor::ui::TrendRange::H24,
        trend_metric: tui_token_monitor::ui::TrendMetric::Tokens,
        dim: 0, model_offset: 0, tick: 0, show_help: false,
    };
    let backend = ratatui::backend::TestBackend::new(150, 36);
    let mut term = ratatui::Terminal::new(backend).unwrap();
    let mut hits = tui_token_monitor::ui::HitRegions::default();
    term.draw(|f| tui_token_monitor::ui::render(f, &view, &mut hits)).unwrap();
    for y in 0..36 {
        let mut line = String::new();
        for x in 0..150 {
            line.push(term.backend().buffer()[(x, y)].symbol().to_string().chars().next().unwrap_or(' '));
        }
        println!("{line}");
    }
}
