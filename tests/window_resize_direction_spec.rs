use mica_term::app::windowing::{WindowResizeDirection, parse_resize_direction};

#[test]
fn parse_resize_direction_accepts_all_edges_and_corners() {
    assert_eq!(
        parse_resize_direction("north"),
        Some(WindowResizeDirection::North)
    );
    assert_eq!(
        parse_resize_direction("south"),
        Some(WindowResizeDirection::South)
    );
    assert_eq!(
        parse_resize_direction("east"),
        Some(WindowResizeDirection::East)
    );
    assert_eq!(
        parse_resize_direction("west"),
        Some(WindowResizeDirection::West)
    );
    assert_eq!(
        parse_resize_direction("north-east"),
        Some(WindowResizeDirection::NorthEast)
    );
    assert_eq!(
        parse_resize_direction("north-west"),
        Some(WindowResizeDirection::NorthWest)
    );
    assert_eq!(
        parse_resize_direction("south-east"),
        Some(WindowResizeDirection::SouthEast)
    );
    assert_eq!(
        parse_resize_direction("south-west"),
        Some(WindowResizeDirection::SouthWest)
    );
}

#[test]
fn parse_resize_direction_rejects_unknown_values() {
    assert_eq!(parse_resize_direction("center"), None);
    assert_eq!(parse_resize_direction(""), None);
}
