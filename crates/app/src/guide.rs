//! The guide: a picture drawn faintly over the canvas, to paint by.
//!
//! Another program hands it over with `--guide guide.png` (Duck Mold
//! sends the outline of a model's face, mostly see-through thin lines).
//! It's stretched to the canvas's size, whatever its own, and follows
//! the canvas as it's zoomed, moved and turned. It is only ever drawn on
//! the screen: never painted into a layer, never saved in the `.quill`,
//! never exported. View → Guide hides and shows it.

use std::path::Path;

use eframe::egui::epaint::Vertex;
use eframe::egui::{
    self, Color32, Mesh, Shape, TextureFilter, TextureHandle, TextureId, TextureOptions, pos2,
};
use quill_engine::image::Image;
use quill_engine::view::{Point, View};

use crate::references_view::texture_image;

/// How strongly the guide is drawn: 60% of its own strength, so the
/// painting under its lines can still be seen.
const STRENGTH: f32 = 0.6;

/// Zoomed in, the guide's pixels are sharp squares (its lines stay
/// crisp over a small picture); zoomed out, they're smoothed so thin
/// lines don't flicker away.
const TEXTURE: TextureOptions = TextureOptions {
    magnification: TextureFilter::Nearest,
    ..TextureOptions::LINEAR
};

pub struct Guide {
    image: Image,
    /// The picture on the graphics card, sent the first time it's drawn.
    texture: Option<TextureHandle>,
    /// View → Guide.
    pub shown: bool,
}

impl Guide {
    /// Reads the guide picture (a PNG, or a JPEG). A file that can't be
    /// read gives the reason, in words.
    pub fn load(path: &Path) -> Result<Guide, String> {
        let image = Image::load(path).map_err(|error| error.to_string())?;
        Ok(Guide::from_image(image))
    }

    /// A guide showing `image`.
    pub fn from_image(image: Image) -> Guide {
        Guide {
            image,
            texture: None,
            shown: true,
        }
    }

    /// The guide's picture on the graphics card, once it has been drawn.
    #[cfg(test)]
    pub fn texture_id(&self) -> Option<TextureId> {
        self.texture.as_ref().map(TextureHandle::id)
    }

    /// Draws the guide over a canvas `canvas` pixels wide and high, as
    /// `view` shows it.
    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        view: &View,
        canvas: (u32, u32),
    ) {
        if !self.shown {
            return;
        }
        let texture = self
            .texture
            .get_or_insert_with(|| ctx.load_texture("guide", texture_image(&self.image), TEXTURE));
        painter.add(Shape::mesh(mesh(texture.id(), view, canvas)));
    }
}

/// The guide as a four-cornered shape: the whole picture (`texture`)
/// stretched from corner to corner of the canvas, wherever `view` puts
/// those on screen, at `STRENGTH`.
fn mesh(texture: TextureId, view: &View, (width, height): (u32, u32)) -> Mesh {
    let (width, height) = (width as f32, height as f32);
    let corners = [(0.0, 0.0), (width, 0.0), (width, height), (0.0, height)];
    // Which part of the picture goes at each corner (0..1 across and down).
    let picture_corners = [
        pos2(0.0, 0.0),
        pos2(1.0, 0.0),
        pos2(1.0, 1.0),
        pos2(0.0, 1.0),
    ];
    let mut mesh = Mesh::with_texture(texture);
    for ((x, y), uv) in corners.into_iter().zip(picture_corners) {
        let on_screen = view.canvas_to_screen(Point::new(x, y));
        mesh.vertices.push(Vertex {
            pos: pos2(on_screen.x, on_screen.y),
            uv,
            color: Color32::WHITE.gamma_multiply(STRENGTH),
        });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guide_is_stretched_over_the_canvas_and_follows_the_view() {
        // A canvas 16 x 12, zoomed, moved and turned.
        let mut view = View::default();
        view.pan(Point::new(300.0, 120.0));
        view.zoom_around(Point::new(310.0, 130.0), 20.0);
        view.rotate_around(Point::new(400.0, 300.0), 0.5);
        let mesh = mesh(TextureId::default(), &view, (16, 12));

        // Its corners are the canvas's corners, whatever the guide's own
        // size, and the whole guide picture lies between them.
        let canvas_corners = [(0.0, 0.0), (16.0, 0.0), (16.0, 12.0), (0.0, 12.0)];
        let picture_corners = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        assert_eq!(mesh.vertices.len(), 4);
        for (vertex, (corner, uv)) in mesh
            .vertices
            .iter()
            .zip(canvas_corners.into_iter().zip(picture_corners))
        {
            let expected = view.canvas_to_screen(Point::new(corner.0, corner.1));
            assert_eq!((vertex.pos.x, vertex.pos.y), (expected.x, expected.y));
            assert_eq!((vertex.uv.x, vertex.uv.y), uv);
            // Faint: 60% of its own strength.
            assert_eq!(vertex.color.a(), 153);
        }
        assert_eq!(mesh.indices, [0, 1, 2, 0, 2, 3], "two triangles");
        // Sharp pixels when zoomed in.
        assert_eq!(TEXTURE.magnification, TextureFilter::Nearest);
    }

    #[test]
    fn a_guide_that_cannot_be_read_is_a_message_not_a_crash() {
        let folder = std::env::temp_dir().join(format!("quill-guide-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        assert!(Guide::load(&folder.join("missing.png")).is_err());
        std::fs::write(folder.join("words.png"), "not a picture").unwrap();
        let problem = Guide::load(&folder.join("words.png")).err().unwrap();
        assert!(problem.contains("damaged"), "{problem}");

        // A real one is read, and shown to begin with.
        let mut bytes = Vec::new();
        Image::new(8, 8, [0, 0, 0, 90])
            .write_png(&mut bytes, true)
            .unwrap();
        std::fs::write(folder.join("guide.png"), bytes).unwrap();
        let guide = Guide::load(&folder.join("guide.png")).unwrap();
        assert!(guide.shown);
        assert_eq!((guide.image.width(), guide.image.height()), (8, 8));
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
