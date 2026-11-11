//! `<name>.regions.json`: the named rectangles a script declares with
//! `region` and `skin_box`, written beside the picture so a game reads
//! the layout the script painted by (`docs/sprite-sheet-format.md`,
//! "Regions"). Only written when the script declares some.

use std::path::{Path, PathBuf};

use quill_engine::script::Report;
use serde_json::{Map, Value, json};

use crate::{Problem, failed};

/// The JSON for `report`'s regions, or `None` if it has none.
///
/// `image` is the picture's file name, `size` its size and `scale` how
/// many times it was enlarged (regions are given in the picture's own
/// pixels, so already multiplied). `frame` is one cell's size when the
/// picture is a sheet of frames or tiles: the regions are then inside
/// each cell, measured from the cell's top-left corner.
pub fn regions_json(
    report: &Report,
    image: &str,
    size: (u32, u32),
    scale: u32,
    frame: Option<(u32, u32)>,
) -> Option<String> {
    if report.regions.is_empty() {
        return None;
    }
    let s = scale.max(1);
    let mut regions = Map::new();
    for region in &report.regions {
        regions.insert(
            region.name.clone(),
            json!({
                "x": region.x * s as i32,
                "y": region.y * s as i32,
                "width": region.width * s,
                "height": region.height * s,
            }),
        );
    }
    let mut description = json!({
        "image": image,
        "width": size.0,
        "height": size.1,
        "scale": s,
    });
    if let Some((width, height)) = frame {
        description["frame"] = json!({ "width": width, "height": height });
    }
    description["regions"] = Value::Object(regions);
    if !report.skin_boxes.is_empty() {
        let mut boxes = Map::new();
        for skin_box in &report.skin_boxes {
            let [w, h, d] = skin_box.size;
            let faces: Vec<String> = quill_engine::script::SkinBox::FACES
                .iter()
                .map(|face| format!("{}.{face}", skin_box.name))
                .collect();
            boxes.insert(
                skin_box.name.clone(),
                json!({
                    "x": skin_box.at[0] * s as i32,
                    "y": skin_box.at[1] * s as i32,
                    "size": [w * s, h * s, d * s],
                    "faces": faces,
                }),
            );
        }
        description["boxes"] = Value::Object(boxes);
    }
    Some(serde_json::to_string_pretty(&description).unwrap_or_default() + "\n")
}

/// Writes `<stem>.regions.json` into `out` if the script declared
/// regions; returns its path if it did.
pub fn save_regions(
    report: &Report,
    out: &Path,
    stem: &str,
    image: &str,
    size: (u32, u32),
    scale: u32,
    frame: Option<(u32, u32)>,
) -> Result<Option<PathBuf>, Problem> {
    let Some(text) = regions_json(report, image, size, scale, frame) else {
        return Ok(None);
    };
    let path = out.join(format!("{stem}.regions.json"));
    std::fs::write(&path, text)
        .map_err(|e| failed(format!("couldn't save {}: {e}", path.display())))?;
    Ok(Some(path))
}
