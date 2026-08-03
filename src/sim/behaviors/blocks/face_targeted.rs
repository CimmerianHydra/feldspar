use serde::Deserialize;

use crate::content::block::behaviors::BlockBehavior;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// FACE TARGETED
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Declares that this block's interactions are addressed by **target face**
/// rather than by the face the ray happened to strike.
///
/// Named for the interaction, not for the overlay it produces. The 3×3 grid
/// the renderer draws is what this *looks like*; what it *means* is that
/// `BlockEntityEvent::target_face` may differ from `face`. Presentation
/// observing this is the same inversion as `ui::inventory` observing
/// `Inventory` — simulation declares a capability, the renderer notices.
///
/// Independent of `Orientable` in both directions:
/// - a pipe is face-targeted and *not* orientable (corners toggle a
///   connection on the far side, they do not spin the pipe);
/// - a four-facing furnace is orientable and *not* face-targeted, because a
///   grid on a block with four legal orientations is noise.
///
/// ## Why this can live here now
///
/// It used to be pinned to `content` because `BlockComponents` had a
/// `face_targeted: Option<FaceTargeted>` field and therefore had to name the
/// type. The bag is a type-erased list now, so the layer that *uses* a
/// capability is free to own it. `render::highlight` and
/// `player::interaction` both sit above `sim`, so they read it from here.
#[derive(Clone, Copy, Default, Debug, Deserialize)]
pub struct FaceTargeted;

impl BlockBehavior for FaceTargeted {
    const NAME: &'static str = "face_targeted";
}
