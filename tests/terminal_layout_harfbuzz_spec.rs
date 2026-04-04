use mica_term::app::terminal_font::mock::mock_font_system;
use mica_term::app::terminal_font::{
    FontRequest, FontSystem, OpenTypeFeatureSet, TextShapingRequest,
};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::DirectWriteFontSystem;
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

fn shape_row_with_mock_font(
    row: &TerminalModelRow,
) -> anyhow::Result<mica_term::app::terminal_layout::ShapedRow> {
    let mut fonts = mock_font_system();
    let loaded_font = fonts.load_font(&FontRequest::default())?;
    shape_row(row, &loaded_font, &mut fonts)
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

    let shaped = shape_row_with_mock_font(&row)?;

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

    let shaped = shape_row_with_mock_font(&row)?;
    let clusters = unique_clusters(&shaped);

    assert_eq!(shaped.runs.len(), 1);
    assert_eq!(shaped.runs[0].cell_range, 0..4);
    assert_eq!(clusters.len(), 2);
    Ok(())
}

#[test]
fn harfbuzz_layout_splits_runs_when_foreground_or_background_changes() -> anyhow::Result<()> {
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

    let shaped = shape_row_with_mock_font(&row)?;

    assert_eq!(shaped.runs.len(), 4);
    assert_eq!(shaped.runs[0].cell_range, 0..1);
    assert_eq!(shaped.runs[1].cell_range, 1..2);
    assert_eq!(shaped.runs[2].cell_range, 2..3);
    assert_eq!(shaped.runs[3].cell_range, 3..4);
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

    let shaped = shape_row_with_mock_font(&row)?;
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

    let shaped = shape_row_with_mock_font(&row)?;

    assert_eq!(shaped.runs.len(), 3);
    assert!(!shaped.runs[0].style.bold);
    assert!(shaped.runs[1].style.bold);
    assert!(!shaped.runs[1].style.underline);
    assert!(shaped.runs[2].style.bold);
    assert!(shaped.runs[2].style.underline);
    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn dwrite_font_system_discovers_distinct_fallback_faces_for_mixed_text() -> anyhow::Result<()> {
    let mut fonts = DirectWriteFontSystem::new()?;
    let loaded_font = fonts.load_font(&FontRequest::default())?;
    let fallback_faces = fonts.discover_fallback_faces(&loaded_font, "A🙂⌘界")?;
    let primary_family = loaded_font
        .family_name()
        .expect("default font request should expose a primary family")
        .to_string();
    let distinct_families = fallback_faces
        .iter()
        .map(|face| face.family_name.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let distinct_face_keys = fallback_faces
        .iter()
        .map(|face| face.face_key.0)
        .collect::<std::collections::BTreeSet<_>>();

    assert!(
        !fallback_faces.is_empty(),
        "mixed text should yield at least the primary face"
    );
    assert_eq!(
        fallback_faces.first().map(|face| face.family_name.as_str()),
        Some(primary_family.as_str()),
        "fallback discovery should keep the primary family first"
    );
    assert!(
        fallback_faces
            .iter()
            .any(|face| face.family_name != primary_family),
        "mixed text should discover at least one fallback family distinct from the primary family"
    );
    assert_eq!(
        distinct_families.len(),
        fallback_faces.len(),
        "fallback discovery should not emit duplicate families"
    );
    assert_eq!(
        distinct_face_keys.len(),
        fallback_faces.len(),
        "fallback discovery should assign a stable face key to each resolved fallback face instead of reusing one synthetic key across the whole chain"
    );
    assert!(
        fallback_faces.len() >= 2,
        "mixed text should resolve to multiple families instead of a single bundled family"
    );
    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn dwrite_shape_text_runs_split_mixed_text_and_preserve_feature_request() -> anyhow::Result<()> {
    let mut fonts = DirectWriteFontSystem::new()?;
    let loaded_font = fonts.load_font(&FontRequest::default())?;
    let primary_family = loaded_font
        .family_name()
        .expect("default font request should expose a primary family")
        .to_string();
    let request = TextShapingRequest {
        text: "A🙂⌘界".into(),
        feature_set: OpenTypeFeatureSet {
            feature_tags: vec!["ss01".into(), "calt".into()],
        },
        allow_ligatures: false,
    };

    let shaped_runs = fonts.shape_text_runs(&loaded_font, &request)?;

    assert!(
        shaped_runs.len() >= 2,
        "mixed text should shape into multiple fallback-aware runs"
    );
    assert_eq!(
        shaped_runs.iter().map(|run| run.text.as_str()).collect::<String>(),
        request.text,
        "fallback-aware shaping should preserve the original text payload across subruns"
    );
    assert!(
        shaped_runs
            .iter()
            .all(|run| run.feature_set == request.feature_set),
        "backend shaping should preserve the explicit OpenType feature request across all subruns"
    );
    assert!(
        shaped_runs.iter().all(|run| !run.allow_ligatures),
        "backend shaping should honor explicit ligature disable requests"
    );
    assert!(
        shaped_runs
            .iter()
            .any(|run| run.resolved_face.family_name != primary_family),
        "mixed text should resolve at least one subrun through a non-primary fallback face"
    );
    assert!(
        shaped_runs
            .iter()
            .map(|run| run.resolved_face.face_key.0)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            >= 2,
        "mixed text should keep distinct fallback face keys once the backend resolves real per-face data"
    );
    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn harfbuzz_layout_maps_fallback_subruns_back_to_terminal_cells() -> anyhow::Result<()> {
    let row = build_row(
        vec![
            TerminalModelCell {
                row: 0,
                col: 0,
                width: 1,
                text: "A".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 1,
                width: 2,
                text: "🙂".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 3,
                width: 1,
                text: "⌘".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
            TerminalModelCell {
                row: 0,
                col: 4,
                width: 2,
                text: "界".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            },
        ],
        "A🙂⌘界",
    );

    let mut fonts = DirectWriteFontSystem::new()?;
    let loaded_font = fonts.load_font(&FontRequest::default())?;
    let shaped = shape_row(&row, &loaded_font, &mut fonts)?;

    assert!(
        shaped.runs.len() >= 2,
        "mixed text layout should expose multiple fallback-aware runs"
    );
    assert_eq!(
        shaped.runs.iter().map(|run| run.text.as_str()).collect::<String>(),
        row.text,
        "layout fallback runs should preserve the source row text"
    );
    assert_eq!(
        shaped.runs.first().map(|run| run.cell_range.start),
        Some(0),
        "first fallback run should begin at the first terminal cell"
    );
    assert_eq!(
        shaped.runs.last().map(|run| run.cell_range.end),
        Some(6),
        "last fallback run should end at the last occupied terminal cell"
    );
    assert!(
        shaped
            .runs
            .windows(2)
            .all(|pair| pair[0].cell_range.end == pair[1].cell_range.start),
        "fallback runs should map onto contiguous terminal cell ranges without overlap"
    );
    Ok(())
}
