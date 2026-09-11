use anyhow::Result;
use poqi_store::{settings, Store};
use poqi_ui::{
    UiRuntimeSettings, UiRuntimeSettingsParts, DEFAULT_FAST_SCROLL_STEP, DEFAULT_MAIN_TICK_RATE_MS,
    DEFAULT_MENU_TICK_RATE_MS, DEFAULT_MIN_COLUMN_WIDTH, DEFAULT_MOUSE_SCROLL_LINES,
    DEFAULT_SELECT_TOP_LIMIT, DEFAULT_STATUS_AUTO_CLEAR_SECS, DEFAULT_STATUS_EXPIRE_SECS,
};

pub fn load_ui_settings(store: &Store) -> Result<UiRuntimeSettings> {
    let repo = store.settings();
    repo.ensure_setting(
        settings::KEY_UI_MAIN_TICK_RATE_MS,
        &DEFAULT_MAIN_TICK_RATE_MS,
    )?;
    repo.ensure_setting(
        settings::KEY_UI_MENU_TICK_RATE_MS,
        &DEFAULT_MENU_TICK_RATE_MS,
    )?;
    repo.ensure_setting(settings::KEY_UI_FAST_SCROLL_STEP, &DEFAULT_FAST_SCROLL_STEP)?;
    repo.ensure_setting(
        settings::KEY_UI_MOUSE_SCROLL_LINES,
        &DEFAULT_MOUSE_SCROLL_LINES,
    )?;
    repo.ensure_setting(settings::KEY_UI_SELECT_TOP_LIMIT, &DEFAULT_SELECT_TOP_LIMIT)?;
    repo.ensure_setting(
        settings::KEY_UI_STATUS_EXPIRE_SECS,
        &DEFAULT_STATUS_EXPIRE_SECS,
    )?;
    repo.ensure_setting(
        settings::KEY_UI_STATUS_AUTO_CLEAR_SECS,
        &DEFAULT_STATUS_AUTO_CLEAR_SECS,
    )?;
    repo.ensure_setting(settings::KEY_UI_MIN_COLUMN_WIDTH, &DEFAULT_MIN_COLUMN_WIDTH)?;

    let main_tick = repo
        .main_tick_rate_ms()?
        .unwrap_or(DEFAULT_MAIN_TICK_RATE_MS);
    let menu_tick = repo
        .menu_tick_rate_ms()?
        .unwrap_or(DEFAULT_MENU_TICK_RATE_MS);
    let fast_scroll = repo.fast_scroll_step()?.unwrap_or(DEFAULT_FAST_SCROLL_STEP);
    let mouse_lines = repo
        .mouse_scroll_lines()?
        .unwrap_or(DEFAULT_MOUSE_SCROLL_LINES);
    let select_top_limit = repo.select_top_limit()?.unwrap_or(DEFAULT_SELECT_TOP_LIMIT);
    let status_expire = repo
        .status_expire_secs()?
        .unwrap_or(DEFAULT_STATUS_EXPIRE_SECS);
    let status_auto_clear = repo
        .status_auto_clear_secs()?
        .unwrap_or(DEFAULT_STATUS_AUTO_CLEAR_SECS);
    let min_column_width = repo.min_column_width()?.unwrap_or(DEFAULT_MIN_COLUMN_WIDTH);

    let parts = UiRuntimeSettingsParts {
        main_tick_rate_ms: main_tick,
        menu_tick_rate_ms: menu_tick,
        fast_scroll_step: fast_scroll,
        mouse_scroll_lines: mouse_lines,
        select_top_limit,
        status_expire_secs: status_expire,
        status_auto_clear_secs: status_auto_clear,
        min_column_width,
    };

    Ok(UiRuntimeSettings::from_parts(parts))
}
