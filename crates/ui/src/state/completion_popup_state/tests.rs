use super::CompletionPopup;
use poqi_search_completion::{CompletionItem, CompletionKind};
use smallvec::SmallVec;

fn demo_item(label: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        insert_text: label.to_string(),
        detail: String::new(),
        kind: CompletionKind::Keyword,
        score: 0,
    }
}

#[test]
fn selection_moves_within_bounds() {
    let mut popup = CompletionPopup::new();
    popup.set_items(SmallVec::from_vec(vec![
        demo_item("a"),
        demo_item("b"),
        demo_item("c"),
    ]));
    assert_eq!(popup.selected_index(), None);
    popup.select_next();
    assert_eq!(popup.selected_index(), Some(0));
    popup.select_next();
    assert_eq!(popup.selected_index(), Some(1));
    popup.select_next();
    assert_eq!(popup.selected_index(), Some(2));
    popup.select_previous();
    assert_eq!(popup.selected_index(), Some(1));
    popup.select_previous();
    assert_eq!(popup.selected_index(), Some(0));
    popup.select_previous();
    assert_eq!(popup.selected_index(), Some(0));
}

#[test]
fn clear_hides_items() {
    let mut popup = CompletionPopup::new();
    popup.set_items(SmallVec::from_vec(vec![demo_item("a")]));
    assert!(popup.is_visible());
    popup.clear();
    assert!(!popup.is_visible());
    assert!(popup.selected_index().is_none());
}

#[test]
fn viewport_resize_tracks_selection() {
    let mut popup = CompletionPopup::new();
    popup.set_items(SmallVec::from_vec(vec![
        demo_item("a"),
        demo_item("b"),
        demo_item("c"),
        demo_item("d"),
        demo_item("e"),
    ]));
    popup.set_viewport_rows(2);
    popup.set_selected_index(0);
    popup.set_selected_index(4);
    let (start, end) = popup.visible_range();
    assert_eq!((start, end), (3, 5));
    assert_eq!(popup.selected_index(), Some(4));
}

#[test]
fn scrolling_adjusts_highlight_into_view() {
    let mut popup = CompletionPopup::new();
    popup.set_items(SmallVec::from_vec(vec![
        demo_item("a"),
        demo_item("b"),
        demo_item("c"),
        demo_item("d"),
    ]));
    popup.set_viewport_rows(2);
    popup.set_selected_index(0);
    popup.scroll_viewport(1);
    let (start, end) = popup.visible_range();
    assert_eq!((start, end), (1, 3));
    assert_eq!(popup.selected_index(), Some(1));
    popup.scroll_viewport(10);
    let (start, end) = popup.visible_range();
    assert_eq!((start, end), (2, 4));
    assert_eq!(popup.selected_index(), Some(2));
}
