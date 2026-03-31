use mica_term::app::terminal_font::mock::mock_font_system;
use mica_term::app::terminal_layout::shape_row;
use mica_term::app::terminal_model::{TerminalModelCell, TerminalModelRow};

fn build_row(cells: Vec<TerminalModelCell>, text: &str) -> TerminalModelRow {
    TerminalModelRow {
        row_index: 0,
        text: text.into(),
        wrapped: false,
        cells,
        row_hash: 0,
    }
}

fn unique_clusters(row: &mica_term::app::terminal_layout::ShapedRow) -> Vec<u32> {
    let mut values = row
        .runs
        .iter()
        .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.cluster))
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    values
}

#[test]
fn harfbuzz_layout_keeps_ascii_prompt_in_one_run_when_style_is_consistent() -> anyhow::Result<()> {
    let row = build_row(
        vec![
            TerminalModelCell {
                row: 0,
                col: 0,
                width: 1,
                text: "$".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 1,
                width: 1,
                text: " ".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 2,
                width: 1,
                text: "p".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 3,
                width: 1,
                text: "w".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 4,
                width: 1,
                text: "d".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
        ],
        "$ pwd",
    );

    let shaped = shape_row(&row, &mut mock_font_system())?;

    assert_eq!(shaped.runs.len(), 1);
    assert_eq!(shaped.runs[0].cell_range, 0..5);
    assert_eq!(shaped.runs[0].text, "$ pwd");
    assert!(shaped.runs[0].glyphs.len() >= 5);
    Ok(())
}

#[test]
fn harfbuzz_layout_keeps_wide_cjk_and_emoji_cluster_boundaries_stable() -> anyhow::Result<()> {
    let row = build_row(
        vec![
            TerminalModelCell {
                row: 0,
                col: 0,
                width: 2,
                text: "界".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 2,
                width: 2,
                text: "🙂".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
        ],
        "界🙂",
    );

    let shaped = shape_row(&row, &mut mock_font_system())?;
    let clusters = unique_clusters(&shaped);

    assert_eq!(shaped.runs.len(), 1);
    assert_eq!(shaped.runs[0].cell_range, 0..4);
    assert_eq!(clusters.len(), 2);
    Ok(())
}

#[test]
fn harfbuzz_layout_splits_on_foreground_change_but_not_background_change() -> anyhow::Result<()> {
    let row = build_row(
        vec![
            TerminalModelCell {
                row: 0,
                col: 0,
                width: 1,
                text: "a".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 1,
                width: 1,
                text: "b".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff101820,
            },
            TerminalModelCell {
                row: 0,
                col: 2,
                width: 1,
                text: "c".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 3,
                width: 1,
                text: "d".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xff7aa2f7,
                bg_rgba: 0xff0c_1014,
            },
        ],
        "abcd",
    );

    let shaped = shape_row(&row, &mut mock_font_system())?;

    assert_eq!(shaped.runs.len(), 2);
    assert_eq!(shaped.runs[0].cell_range, 0..3);
    assert_eq!(shaped.runs[1].cell_range, 3..4);
    Ok(())
}

#[test]
fn harfbuzz_layout_clusters_combining_sequences_instead_of_iterating_raw_chars()
-> anyhow::Result<()> {
    let row = build_row(
        vec![TerminalModelCell {
            row: 0,
            col: 0,
            width: 1,
            text: "A\u{0301}".into(),
            bold: false,
            underline: false,
            fg_rgba: 0xffd8_dfe8,
            bg_rgba: 0xff0c_1014,
        }],
        "A\u{0301}",
    );

    let shaped = shape_row(&row, &mut mock_font_system())?;
    let clusters = unique_clusters(&shaped);

    assert_eq!(shaped.runs.len(), 1);
    assert_eq!(row.text.chars().count(), 2);
    assert_eq!(clusters.len(), 1);
    Ok(())
}

#[test]
fn harfbuzz_layout_splits_runs_when_bold_or_underline_changes() -> anyhow::Result<()> {
    let row = build_row(
        vec![
            TerminalModelCell {
                row: 0,
                col: 0,
                width: 1,
                text: "a".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 1,
                width: 1,
                text: "b".into(),
                bold: true,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 2,
                width: 1,
                text: "c".into(),
                bold: true,
                underline: true,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
        ],
        "abc",
    );

    let shaped = shape_row(&row, &mut mock_font_system())?;

    assert_eq!(shaped.runs.len(), 3);
    assert!(!shaped.runs[0].style.bold);
    assert!(shaped.runs[1].style.bold);
    assert!(!shaped.runs[1].style.underline);
    assert!(shaped.runs[2].style.bold);
    assert!(shaped.runs[2].style.underline);
    Ok(())
}
