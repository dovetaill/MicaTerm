use mica_term::app::ssh::runtime::{
    TerminalCellState, TerminalCursorShape, TerminalCursorState, TerminalRowState,
    TerminalSurfaceState,
};
use mica_term::app::terminal_model::TerminalModelFrame;
use uuid::Uuid;

fn build_surface(
    seqno: usize,
    rows: u32,
    cols: u32,
    row_texts: &[&str],
    cells: Vec<TerminalCellState>,
) -> TerminalSurfaceState {
    let session_id = Uuid::new_v4();
    let mut surface = TerminalSurfaceState::from_visible_lines(
        session_id,
        seqno,
        rows,
        cols,
        row_texts.iter().map(|value| value.to_string()).collect(),
    );
    surface.visible_rows = row_texts
        .iter()
        .enumerate()
        .map(|(index, text)| TerminalRowState {
            index: index as u32,
            text: (*text).to_string(),
            wrapped: index == 1,
        })
        .collect();
    surface.visible_lines = row_texts.iter().map(|value| value.to_string()).collect();
    surface.cells = cells;
    surface
}

#[test]
fn terminal_model_preserves_visible_row_text_and_color_spans() {
    let surface = build_surface(
        7,
        2,
        8,
        &["a界🙂", "wrap"],
        vec![
            TerminalCellState {
                row: 0,
                col: 0,
                width: 1,
                text: "a".into(),
                fg_rgba: 0xff11_2233,
                bg_rgba: 0xff00_0000,
            },
            TerminalCellState {
                row: 0,
                col: 1,
                width: 2,
                text: "界".into(),
                fg_rgba: 0xff44_5566,
                bg_rgba: 0xff00_0000,
            },
            TerminalCellState {
                row: 0,
                col: 3,
                width: 2,
                text: "🙂".into(),
                fg_rgba: 0xff77_8899,
                bg_rgba: 0xff00_1111,
            },
        ],
    );

    let frame = TerminalModelFrame::from_surface(&surface, None);

    assert_eq!(frame.rows.len(), 2);
    assert_eq!(frame.rows[0].row_index, 0);
    assert_eq!(frame.rows[0].text, "a界🙂");
    assert_eq!(frame.rows[1].text, "wrap");
    assert!(frame.rows[1].wrapped);
    assert_eq!(frame.rows[0].cells.len(), 3);
    assert_eq!(frame.rows[0].cells[1].text, "界");
    assert_eq!(frame.rows[0].cells[1].width, 2);
    assert_eq!(frame.rows[0].cells[1].fg_rgba, 0xff44_5566);
    assert_eq!(frame.rows[0].cells[2].bg_rgba, 0xff00_1111);
}

#[test]
fn terminal_model_preserves_cursor_and_current_selection_contract() {
    let mut surface = build_surface(
        3,
        2,
        8,
        &["cursor", ""],
        vec![TerminalCellState {
            row: 0,
            col: 0,
            width: 1,
            text: "c".into(),
            fg_rgba: 0xffde_adbe,
            bg_rgba: 0xff10_1010,
        }],
    );
    surface.default_fg_rgba = 0xffab_cdef;
    surface.default_bg_rgba = 0xff01_0203;
    surface.row_bg_even_rgba = 0xff11_1111;
    surface.row_bg_odd_rgba = 0xff22_2222;
    surface.cursor = TerminalCursorState {
        row: 1,
        col: 4,
        visible: true,
        blinking: true,
        shape: TerminalCursorShape::Underline,
        fg_rgba: 0xffcc_dd11,
        bg_rgba: 0xff22_3344,
    };

    let frame = TerminalModelFrame::from_surface(&surface, None);

    assert_eq!(frame.cursor.row, 1);
    assert_eq!(frame.cursor.col, 4);
    assert!(frame.cursor.visible);
    assert!(frame.cursor.blinking);
    assert_eq!(frame.cursor.shape, TerminalCursorShape::Underline);
    assert_eq!(frame.cursor.fg_rgba, 0xffcc_dd11);
    assert_eq!(frame.cursor.bg_rgba, 0xff22_3344);
    assert!(frame.selection.is_none());
    assert_eq!(frame.palette.default_fg_rgba, 0xffab_cdef);
    assert_eq!(frame.palette.default_bg_rgba, 0xff01_0203);
    assert_eq!(frame.palette.row_bg_even_rgba, 0xff11_1111);
    assert_eq!(frame.palette.row_bg_odd_rgba, 0xff22_2222);
}

#[test]
fn terminal_model_marks_only_changed_rows_dirty() {
    let session_id = Uuid::new_v4();
    let mut surface_a = TerminalSurfaceState::from_visible_lines(
        session_id,
        1,
        5,
        8,
        vec![
            "row0".into(),
            "row1".into(),
            "row2".into(),
            "row3".into(),
            "row4".into(),
        ],
    );
    surface_a.visible_rows = (0..5)
        .map(|index| TerminalRowState {
            index,
            text: format!("row{index}"),
            wrapped: false,
        })
        .collect();
    surface_a.cells = vec![
        TerminalCellState {
            row: 3,
            col: 0,
            width: 1,
            text: "r".into(),
            fg_rgba: 0xffaa_aaaa,
            bg_rgba: 0xff00_0000,
        },
        TerminalCellState {
            row: 4,
            col: 0,
            width: 1,
            text: "r".into(),
            fg_rgba: 0xffbb_bbbb,
            bg_rgba: 0xff00_0000,
        },
    ];

    let mut surface_b = surface_a.clone();
    surface_b.seqno = 2;
    surface_b.visible_rows[3].text = "row3 changed".into();
    surface_b.visible_lines[3] = "row3 changed".into();
    surface_b.cells[1].fg_rgba = 0xffcc_cccc;

    let previous = TerminalModelFrame::from_surface(&surface_a, None);
    let next = TerminalModelFrame::from_surface(&surface_b, Some(&previous));

    assert_eq!(next.dirty_rows, vec![3, 4]);
}
